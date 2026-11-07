//! Release and cleanup emission, mirrored from codegen.zig's
//! `emitRelease` cascade (841-1110), the cleanup-list emitters
//! (1361-1500) and `emitForgetMovedValue` (9766-9780).
//!
//! The Zig cascade probes a series of handle-family tables before the
//! plain `!name` default (mutex guards, RefCell borrow handles, file
//! and metadata handles, mpsc/string/collection binding sets). The
//! slice maintains none of those tables — no fixture program creates
//! such a binding — so those probes are structurally absent here and
//! the cascade reduces to the guards the slice can hit, in the Zig
//! order: internal symbol, `&`/`^` prefix, stack-alloc binding, global
//! const, already-consumed, then the default release. Adding a handle
//! family means adding its probe at the Zig position, not appending.

use std::collections::HashSet;

use sla_ast::Node;
use sla_lowering::{
    is_borrow_like_type, is_internal_symbol, plan_function_tail_cleanup, root_identifier,
    FunctionTailCleanupAction,
};

use crate::codegen::Codegen;
use crate::Result;

impl<'c> Codegen<'c> {
    /// Snapshot/restore of the consumed set, the slice-visible part of
    /// the Zig branch-state save/restore (which also snapshots borrow
    /// and RefCell state the slice does not track).
    pub(crate) fn snapshot_consumed(&self) -> HashSet<String> {
        self.consumed_bindings.clone()
    }

    pub(crate) fn restore_consumed(&mut self, snapshot: &HashSet<String>) {
        self.consumed_bindings = snapshot.clone();
    }

    /// codegen.zig:841 `emitRelease`.
    pub(crate) fn emit_release(&mut self, name: &str) -> Result<()> {
        let resolved = self.resolve_binding_name(name);
        if is_internal_symbol(&resolved) {
            return Ok(());
        }
        // (Zig: borrow-address-temp release plan over
        // `borrow_source_temps` — empty for every slice program.)
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
        self.consumed_bindings.insert(resolved);
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
