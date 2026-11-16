//! Release and cleanup emission, mirrored from codegen.zig's
//! `emitRelease` cascade (841-1110), the cleanup-list emitters
//! (1361-1500) and `emitForgetMovedValue` (9766-9780).
//!
//! The Zig cascade probes a series of handle-family tables before the
//! plain `!name` default (mutex guards, RefCell borrow handles, file
//! and metadata handles, mpsc/string/collection binding sets). The
//! slice maintains only one of those tables — `borrow_source_temps`,
//! populated by the place-address consumers (place.rs) — so the other
//! probes are structurally absent here and the cascade reduces to
//! the guards the slice can hit, in the Zig order: internal symbol,
//! borrow-address-temp plan, `&`/`^` prefix, stack-alloc binding,
//! global const, already-consumed, then the default release. Adding
//! a handle family means adding its probe at the Zig position, not
//! appending.

use std::collections::{HashMap, HashSet};

use sla_ast::Node;
use sla_lowering::{
    is_borrow_like_type, is_internal_symbol, plan_function_tail_cleanup, root_identifier,
    FunctionTailCleanupAction,
};

use crate::codegen::Codegen;
use crate::Result;

impl<'c> Codegen<'c> {
    /// Snapshot/restore of the consumed set and the borrow-source
    /// table, the slice-visible part of the Zig branch-state
    /// save/restore (codegen.zig:2417-2452; RefCell state is not
    /// tracked in this lane).
    pub(crate) fn snapshot_consumed(&self) -> (HashSet<String>, HashMap<String, String>) {
        (
            self.consumed_bindings.clone(),
            self.borrow_source_temps.clone(),
        )
    }

    pub(crate) fn restore_consumed(
        &mut self,
        snapshot: &(HashSet<String>, HashMap<String, String>),
    ) {
        self.consumed_bindings = snapshot.0.clone();
        self.borrow_source_temps = snapshot.1.clone();
    }

    /// codegen.zig:14942 (the borrow arm of transferValueState):
    /// a binding that takes over a borrow-address temp inherits
    /// its source temp, so releasing the binding settles both.
    pub(crate) fn transfer_borrow_source_to_binding(&mut self, value_reg: &str, binding: &str) {
        if let Some(source_temp) = self.borrow_source_temps.remove(value_reg) {
            self.borrow_source_temps
                .insert(binding.to_string(), source_temp);
        }
    }

    /// codegen.zig:1136 `flushMutexGuardsForReceiver`.
    pub(crate) fn flush_mutex_guards_for_receiver(&mut self, mutex_reg: &str) -> Result<()> {
        if !self.consumed_bindings.contains(mutex_reg) {
            return Ok(());
        }
        let mut guard_names: Vec<String> = Vec::new();
        for (name, reg) in &self.mutex_guard_handles {
            if reg == mutex_reg {
                guard_names.push(name.clone());
            }
        }
        for (name, reg) in &self.mutex_lock_results {
            if reg == mutex_reg {
                guard_names.push(name.clone());
            }
        }
        for name in guard_names {
            self.emit_release(&name)?;
        }
        Ok(())
    }

    /// codegen.zig:2253 `releaseTemporaryHandleRegister`: a
    /// temporary handle register (a `tmp_N` name) is retired with
    /// a plain `!`; a live binding is left for its own release.
    pub(crate) fn release_temporary_handle_register(&mut self, handle_reg: &str) -> Result<()> {
        if !handle_reg.starts_with("tmp_") {
            return Ok(());
        }
        if self.consumed_bindings.contains(handle_reg) {
            return Ok(());
        }
        self.ins(&format!("!{handle_reg}"));
        self.consumed_bindings.insert(handle_reg.to_string());
        Ok(())
    }

    /// codegen.zig:2222 `restoreMutexReceiverAfterGuardRelease`:
    /// once no guard or lock Result references the mutex any
    /// more, the receiver is consumable again, so its
    /// scope-exit release is not suppressed.
    pub(crate) fn restore_mutex_receiver_after_guard_release(&mut self, mutex_reg: &str) {
        if self
            .mutex_guard_handles
            .values()
            .any(|reg| reg == mutex_reg)
        {
            return;
        }
        if self
            .mutex_lock_results
            .values()
            .any(|reg| reg == mutex_reg)
        {
            return;
        }
        self.consumed_bindings.remove(mutex_reg);
    }

    /// codegen.zig:841 `emitRelease`.
    pub(crate) fn emit_release(&mut self, name: &str) -> Result<()> {
        let resolved = self.resolve_binding_name(name);
        if is_internal_symbol(&resolved) {
            return Ok(());
        }
        // codegen.zig:1136 `flushMutexGuardsForReceiver`: a
        // statement cleanup list may visit the mutex receiver
        // before the guards that keep it marked consumed; when
        // the receiver's release is requested in that poisoned
        // state, release its live guards first so the
        // receiver's own release is not suppressed.
        self.flush_mutex_guards_for_receiver(&resolved)?;
        // codegen.zig:1143: the borrow-address-temp release plan
        // (lowering_rules.planBorrowAddressTempRelease): releasing
        // a recorded borrow address settles the borrow register
        // itself, then releases its source temp.
        if let Some(source_temp) = self.borrow_source_temps.remove(&resolved) {
            if !self.consumed_bindings.contains(&resolved) {
                self.consumed_bindings.insert(resolved.clone());
                self.ins(&format!("!{resolved}"));
            }
            self.emit_release(&source_temp)?;
            return Ok(());
        }
        // codegen.zig:1186 `mutex_guard_handles` probe: releasing
        // a live guard unlocks its mutex, retires the guard
        // register, and restores the receiver for its own
        // scope-exit release. (Zig: mutex/rwlock/refcell/file/
        // metadata handle families — only the mutex family is
        // populated in this slice; RwLock/RefCell/file probes
        // belong at their own Zig positions when those families
        // land.)
        if let Some(mutex_reg) = self.mutex_guard_handles.remove(&resolved) {
            self.ins(&format!("EXPAND MUTEX_UNLOCK {mutex_reg}"));
            self.release_temporary_handle_register(&mutex_reg)?;
            self.consumed_bindings.insert(resolved.clone());
            self.ins(&format!("!{resolved}"));
            self.restore_mutex_receiver_after_guard_release(&mutex_reg);
            return Ok(());
        }
        // codegen.zig:1196 `mutex_lock_results` probe: a lock
        // Result dropped without `unwrap` unlocks the same way.
        if let Some(mutex_reg) = self.mutex_lock_results.remove(&resolved) {
            self.ins(&format!("EXPAND MUTEX_UNLOCK {mutex_reg}"));
            self.release_temporary_handle_register(&mutex_reg)?;
            self.consumed_bindings.insert(resolved.clone());
            self.ins(&format!("!{resolved}"));
            self.restore_mutex_receiver_after_guard_release(&mutex_reg);
            return Ok(());
        }
        // (Zig: mutex/rwlock/refcell/file/metadata handle families —
        // no such bindings exist in the slice.)
        if resolved.starts_with('&') || resolved.starts_with('^') {
            return Ok(());
        }
        if self.stack_alloc_bindings.contains(&resolved) {
            return Ok(());
        }
        if self.global_const_bindings.contains(&resolved) {
            return Ok(());
        }
        if self.consumed_bindings.contains(&resolved) {
            return Ok(());
        }
        // (Zig: mpsc sender/receiver, string-buffer and collection
        // binding families emit EXPAND *_FREE — unpopulated here.)
        self.ins(&format!("!{resolved}"));
        self.consumed_bindings.insert(resolved.clone());
        // codegen.zig:1000-1009 (stage 6 async cluster): the
        // async state maps are dropped with the register, and a
        // task's wrapped future object is released with the task.
        self.future_state_vtables.remove(&resolved);
        self.future_readiness.remove(&resolved);
        if let Some(future_obj) = self.task_future_objects.remove(&resolved) {
            if !self.consumed_bindings.contains(&future_obj) {
                self.ins(&format!("!{future_obj}"));
                self.consumed_bindings.insert(future_obj);
            }
        }
        Ok(())
    }

    /// codegen.zig:1050 `emitLexicalCleanupRelease`.
    pub(crate) fn emit_lexical_cleanup_release(&mut self, name: &str) -> Result<()> {
        let resolved = self.resolve_binding_name(name);
        self.emit_release(&resolved)
    }

    /// codegen.zig:1385 `emitFunctionTailCleanups`: release the
    /// statement's checker cleanup list at a tail-expression return,
    /// filtered by the result-cleanup plan.
    pub(crate) fn emit_function_tail_cleanups(
        &mut self,
        stmt: &Node,
        tail_expr: &Node,
    ) -> Result<()> {
        if let Some(list) = self.check.cleanups(stmt) {
            let names: Vec<String> = list.to_vec();
            for name in names {
                if self
                    .plan_function_result_cleanup(&name, tail_expr)?
                    .releases_value()
                {
                    self.emit_release(&name)?;
                }
            }
        }
        Ok(())
    }

    /// codegen.zig:1393 `planFunctionResultCleanup`: the lowering
    /// tail-cleanup plan, widened to a transfer when the cleanup name
    /// is the result's root and the result type is a non-Copy,
    /// non-borrow-like value (the value itself is being returned).
    pub(crate) fn plan_function_result_cleanup(
        &mut self,
        cleanup_name: &str,
        result_expr: &Node,
    ) -> Result<FunctionTailCleanupAction> {
        let base_plan = plan_function_tail_cleanup(cleanup_name, result_expr);
        if base_plan.transfers_result() {
            return Ok(base_plan);
        }
        if let Some(root) = root_identifier(result_expr) {
            if root == cleanup_name {
                if let Some(result_ty) = self.expr_type(result_expr) {
                    if !self.type_is_copy_value(&result_ty)
                        && !is_borrow_like_type(&result_ty)
                    {
                        return Ok(FunctionTailCleanupAction::TransferResult);
                    }
                }
            }
        }
        Ok(base_plan)
    }

    /// codegen.zig `emitBranchScopedCleanupList`: emit a cleanup list,
    /// then restore the consumed set so the sibling branch sees the
    /// pre-branch state.
    pub(crate) fn emit_branch_scoped_cleanup_list(&mut self, list: &[String]) -> Result<()> {
        let snapshot = self.snapshot_consumed();
        for name in list {
            self.emit_release(name)?;
        }
        self.restore_consumed(&snapshot);
        Ok(())
    }

    /// codegen.zig `emitBranchScopedCleanupForNode`.
    pub(crate) fn emit_branch_scoped_cleanup_for_node(&mut self, node: &Node) -> Result<()> {
        if let Some(list) = self.check.cleanups(node) {
            let names: Vec<String> = list.to_vec();
            self.emit_branch_scoped_cleanup_list(&names)?;
        }
        Ok(())
    }

    /// codegen.zig:9766 `emitForgetMovedValue`: the `^name` consume
    /// marker (no output for borrow-prefixed regs).
    pub(crate) fn emit_forget_moved_value(&mut self, reg: &str) -> Result<()> {
        if reg.starts_with('&') {
            return Ok(());
        }
        if let Some(inner) = reg.strip_prefix('^') {
            self.mark_consumed_binding(inner);
            return Ok(());
        }
        self.ins(&format!("^{reg}"));
        self.mark_consumed_binding(reg);
        Ok(())
    }
}
