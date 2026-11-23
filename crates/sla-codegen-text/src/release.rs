//! Release and cleanup emission, mirrored from codegen.zig's
//! `emitRelease` cascade (841-1110), the cleanup-list emitters
//! (1361-1500) and `emitForgetMovedValue` (9766-9780).
//!
//! The Zig cascade probes a series of handle-family tables before the
//! plain `!name` default (mutex guards, RefCell borrow handles, file
//! and metadata handles, mpsc/string/collection binding sets). The
//! slice maintains the `borrow_source_temps` table (populated by the
//! place-address consumers, place.rs), the mutex guard/lock tables
//! and the RefCell borrow-handle table with its result-slot
//! companions; the remaining families' probes are structurally
//! absent here. The cascade keeps the Zig order: internal symbol,
//! borrow-address-temp plan, RefCell borrow handle, mutex guard,
//! mutex lock result, RefCell cell sweep, `&`/`^` prefix,
//! stack-alloc binding (with the result-slot RefCell release),
//! global const, already-consumed, then the default release. Adding
//! a handle family means adding its probe at the Zig position, not
//! appending.

use std::collections::{HashMap, HashSet};

use sla_ast::{Node, Type};
use sla_lowering::{
    is_borrow_like_type, is_internal_symbol, plan_function_tail_cleanup, root_identifier,
    FunctionTailCleanupAction,
};

use crate::codegen::{Codegen, RefCellBorrowHandle};
use crate::Result;

/// SPRINT-100 L2R2: does generating `expr` MOVE the named binding
/// into the value being built? Only the constituents that store
/// the binding register itself count — a bare identifier or a
/// `^identifier`, appearing as the whole expression, as a tuple /
/// struct / enum literal constituent, or as the payload argument
/// of an Option/Result constructor (which stores its argument
/// directly, per the EXPAND constructor forms in call.rs). Any
/// other shape (field/index extraction, a general call argument,
/// a binary operand) may copy or clone, so it never counts.
fn result_moves_binding(expr: &Node, name: &str) -> bool {
    match expr {
        Node::Identifier(n) => n == name,
        Node::MoveExpr(mv) => result_moves_binding(&mv.expr, name),
        Node::TupleLiteral(lit) => lit
            .elements
            .iter()
            .any(|e| result_moves_binding(e, name)),
        Node::StructLiteral(lit) => lit
            .fields
            .iter()
            .any(|f| result_moves_binding(&f.value, name)),
        Node::EnumLiteral(lit) => lit
            .fields
            .iter()
            .any(|f| result_moves_binding(&f.value, name)),
        Node::CallExpr(call) => {
            (call.associated_target.is_none()
                || matches!(
                    call.associated_target.as_deref(),
                    Some("Option") | Some("Result")
                ))
                && matches!(call.func_name.as_str(), "Some" | "Ok" | "Err")
                && call.args.len() == 1
                && result_moves_binding(&call.args[0], name)
        }
        _ => false,
    }
}

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

    /// codegen.zig:1404 `restoreRefCellBranchState`: rebuild
    /// the RefCell borrow-handle table and the borrow-source
    /// table from a snapshot pair (Zig `restoreRefCellBorrowHandles`
    /// codegen.zig:1336 + `restoreBorrowSourceTemps` :1328 —
    /// clear, then copy the snapshot in; here clone-assign,
    /// the same contents). Consumed by the loop emitters under
    /// the loop-state merge plan (lowering_rules.zig:3776):
    /// state after a loop body is the pre-loop state.
    pub(crate) fn restore_refcell_branch_state(
        &mut self,
        handles: &HashMap<String, RefCellBorrowHandle>,
        borrow_sources: &HashMap<String, String>,
    ) -> Result<()> {
        self.refcell_borrow_handles = handles.clone();
        self.borrow_source_temps = borrow_sources.clone();
        Ok(())
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

    /// codegen.zig genLetStmt handle cascade (16100-16133): a
    /// binding that takes over a MutexGuard register inherits its
    /// guard handle (and a lock Result inherits its lock-result
    /// entry), so the binding's own cleanup unlocks the mutex.
    /// Without the transfer the guard released as a plain `!reg`
    /// and the second lock() deadlocked
    /// (test_unit_p1_mutex_helper / test_unit_reg2d3_mutex_test_body).
    pub(crate) fn transfer_mutex_guard_handle(&mut self, value_reg: &str, binding: &str) {
        if let Some(mutex_reg) = self.mutex_guard_handles.remove(value_reg) {
            self.mutex_guard_handles
                .insert(binding.to_string(), mutex_reg);
            self.consumed_bindings.insert(value_reg.to_string());
        }
        if let Some(mutex_reg) = self.mutex_lock_results.remove(value_reg) {
            self.mutex_lock_results
                .insert(binding.to_string(), mutex_reg);
            self.consumed_bindings.insert(value_reg.to_string());
        }
    }

    /// codegen.zig genLetStmt handle cascade (16100-16133): a
    /// binding that takes over a RefCell borrow register inherits
    /// its handle, and the source register is consumed by the
    /// takeover (the binding's own release settles the borrow).
    pub(crate) fn transfer_refcell_borrow_handle(&mut self, value_reg: &str, binding: &str) {
        if let Some(handle) = self.refcell_borrow_handles.remove(value_reg) {
            self.refcell_borrow_handles
                .insert(binding.to_string(), handle);
            self.consumed_bindings.insert(value_reg.to_string());
        }
    }

    /// DEF-SLA-REFCELL-USEAFTERMOVE (task 20): a binding that
    /// takes over a RefCell CELL register inherits the live
    /// borrow handles taken from that cell — their `cell_reg`
    /// is re-keyed onto the new owner, so the handles are
    /// settled through the live register when the new owner is
    /// released. Without this, a move of a borrowed cell
    /// (`let moved_cell = cell`) leaves the handles pointing
    /// at the consumed source, and the first release that
    /// settles one (the return cleanup in a terminating
    /// branch, or the fallthrough `!cell`) loads `cell+8`
    /// from the consumed register (UseAfterMove trap 1009,
    /// test_unit_refcell_owner_release_direct).
    pub(crate) fn transfer_refcell_cell_handles(&mut self, old_cell: &str, binding: &str) {
        for handle in self.refcell_borrow_handles.values_mut() {
            if handle.cell_reg == old_cell {
                handle.cell_reg = binding.to_string();
            }
        }
    }

    /// codegen.zig `prepareResultSlotRefCellCompanion`: a
    /// borrow-typed result slot (planResultSlotTransfer:
    /// needs_refcell_companion == type is a borrow) gets its
    /// companion cell slot allocated up front — before the
    /// branch — so both branch paths can store the borrowed
    /// cell into it and the merged load can restore the handle.
    pub(crate) fn prepare_result_slot_refcell_companion(
        &mut self,
        slot: &str,
        ty: &Type,
    ) {
        if matches!(ty, Type::Borrow(_)) {
            self.ensure_result_slot_refcell_slot(slot);
        }
    }

    /// F1 (RWLOCK panic-17): a binding that takes over a guard
    /// register inherits its RwLock guard handle (and a lock
    /// Result dropped without `unwrap` the same way), so the
    /// binding's own cleanup releases the lock. Without this
    /// the handle stays keyed on the unwrapped temp register
    /// while block-exit cleanup looks it up by binding name —
    /// the probe misses and the lock is never released.
    pub(crate) fn transfer_rwlock_guard_handle(&mut self, value_reg: &str, binding: &str) {
        if let Some(handle) = self.rwlock_guard_handles.remove(value_reg) {
            self.rwlock_guard_handles
                .insert(binding.to_string(), handle);
            self.consumed_bindings.insert(value_reg.to_string());
        }
        if let Some(handle) = self.rwlock_lock_results.remove(value_reg) {
            self.rwlock_lock_results
                .insert(binding.to_string(), handle);
            self.consumed_bindings.insert(value_reg.to_string());
        }
    }

    /// codegen.zig:2258 `ensureResultSlotRefCellSlot`: the
    /// companion slot for a stack slot, allocated on first use.
    pub(crate) fn ensure_result_slot_refcell_slot(&mut self, slot: &str) -> String {
        if let Some(existing) = self.result_slot_refcell_slots.get(slot) {
            return existing.clone();
        }
        let cell_slot = self.new_tmp();
        self.ins(&format!("{cell_slot} = alloc 8"));
        self.result_slot_refcell_slots
            .insert(slot.to_string(), cell_slot.clone());
        cell_slot
    }

    /// codegen.zig:2247 `ensureResultSlotRefCellHandle`.
    pub(crate) fn ensure_result_slot_refcell_handle(
        &mut self,
        slot: &str,
        kind: sla_lowering::refcell::RefCellBorrowKind,
    ) -> crate::codegen::ResultSlotRefCellHandle {
        if let Some(existing) = self.result_slot_refcell_handles.get(slot) {
            let mut updated = existing.clone();
            updated.kind = kind;
            self.result_slot_refcell_handles
                .insert(slot.to_string(), updated.clone());
            return updated;
        }
        let cell_slot = self.ensure_result_slot_refcell_slot(slot);
        let meta = crate::codegen::ResultSlotRefCellHandle { cell_slot, kind };
        self.result_slot_refcell_handles
            .insert(slot.to_string(), meta.clone());
        meta
    }

    /// codegen.zig:2285 `storeResultSlotTransferredValueState`
    /// (the RefCell companion arm): a handle-carrying value
    /// stored into a stack slot leaves its cell register in the
    /// slot's companion slot, so the slot's later release can
    /// settle the borrow. Returns true when a handle moved.
    pub(crate) fn store_refcell_handle_companion_for_slot(
        &mut self,
        slot: &str,
        value_reg: &str,
    ) -> bool {
        let Some(handle) = self.refcell_borrow_handles.remove(value_reg) else {
            return false;
        };
        let meta = self.ensure_result_slot_refcell_handle(slot, handle.kind);
        self.ins(&format!(
            "store {}+0, {} as ptr",
            meta.cell_slot, handle.cell_reg
        ));
        true
    }

    /// codegen.zig:2345 `releaseResultSlotRefCellHandle`: load
    /// the cell register back out of the companion slot, settle
    /// the borrow against it, and (when asked) retire the
    /// companion slot itself.
    pub(crate) fn release_result_slot_refcell_handle(
        &mut self,
        slot: &str,
        remove_companion: bool,
    ) -> Result<bool> {
        let Some(entry) = self.result_slot_refcell_handles.get(slot).cloned() else {
            return Ok(false);
        };
        let cell_reg = self.new_tmp();
        self.ins(&format!("{cell_reg} = load {}+0 as ptr", entry.cell_slot));
        self.ins(&format!(
            "EXPAND {} {cell_reg}",
            entry.kind.release_macro_name()
        ));
        self.emit_release(&cell_reg)?;
        if remove_companion {
            self.result_slot_refcell_handles.remove(slot);
            self.result_slot_refcell_slots.remove(slot);
            self.emit_release(&entry.cell_slot)?;
        }
        Ok(true)
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

    /// codegen.zig:1136 family, RwLock side: when the lock
    /// receiver's release is requested while guards taken from
    /// it are still live, release those guards first.
    pub(crate) fn flush_rwlock_guards_for_receiver(&mut self, lock_reg: &str) -> Result<()> {
        if !self.consumed_bindings.contains(lock_reg) {
            return Ok(());
        }
        let mut guard_names: Vec<String> = Vec::new();
        for (name, handle) in &self.rwlock_guard_handles {
            if handle.lock_reg == lock_reg {
                guard_names.push(name.clone());
            }
        }
        for (name, handle) in &self.rwlock_lock_results {
            if handle.lock_reg == lock_reg {
                guard_names.push(name.clone());
            }
        }
        for name in guard_names {
            self.emit_release(&name)?;
        }
        Ok(())
    }

    /// codegen.zig:2222 family, RwLock side: once no guard or
    /// lock Result references the lock any more, the receiver is
    /// consumable again.
    pub(crate) fn restore_rwlock_receiver_after_guard_release(&mut self, lock_reg: &str) {
        if self
            .rwlock_guard_handles
            .values()
            .any(|h| h.lock_reg == lock_reg)
        {
            return;
        }
        if self
            .rwlock_lock_results
            .values()
            .any(|h| h.lock_reg == lock_reg)
        {
            return;
        }
        self.consumed_bindings.remove(lock_reg);
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
    /// Extern variadic batch (LINE-CENSUS-VARIADIC): Zig
    /// `emitVaEndIfVariadic` — retire the va_start handle
    /// immediately before a return from a variadic definition.
    pub(crate) fn emit_va_end_if_variadic(&mut self) {
        if let Some(list_name) = self.current_variadic_list.clone() {
            self.ins(&format!("va_end {list_name}"));
        }
    }

    pub(crate) fn emit_release(&mut self, name: &str) -> Result<()> {
        // Extern variadic batch (LINE-CENSUS-VARIADIC): the
        // VaList parameter of a variadic definition is not an SA
        // parameter — it is bound by `va_start` and retired by
        // `va_end` — so a checker cleanup list naming it must
        // not release it (the Zig prologue never registers it
        // as a releasable parameter binding).
        if let Some(list_name) = &self.current_variadic_list {
            if name == list_name {
                return Ok(());
            }
        }
        let resolved = self.resolve_binding_name(name);
        if let Some(list_name) = &self.current_variadic_list {
            if resolved == *list_name {
                return Ok(());
            }
        }
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
        // Same flush for RwLock receivers (codegen.zig:1136
        // family).
        self.flush_rwlock_guards_for_receiver(&resolved)?;
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
        // codegen.zig:1261 `refcell_borrow_handles` probe:
        // releasing a live RefCell borrow settles the dynamic
        // borrow against its cell (the RELEASE expansion), then
        // retires the borrow register itself; a tracked owner
        // temp is released last.
        if let Some(handle) = self.refcell_borrow_handles.remove(&resolved) {
            let has_owner_temp = handle
                .cell_release_temp
                .as_deref()
                .map(|temp| temp != resolved)
                .unwrap_or(false);
            let plan = sla_lowering::refcell::plan_refcell_handle_release(has_owner_temp);
            if plan.consume_handle_value {
                self.consumed_bindings.insert(resolved.clone());
            }
            if plan.release_dynamic_borrow {
                // DEF-RUST-REFCELL-BRANCH-HANDLE-MERGE (task 21):
                // a cell-identical handle carries the live cell
                // pointer in its own register, so release against
                // the handle itself. The static cell_reg goes stale
                // when the handle is reassigned in a branch and the
                // if-merge keeps the else end state (the reassigned
                // mut borrow is then left outstanding, panic 107);
                // the handle value tracks the live cell on both
                // paths. Pointer-payload handles keep the static
                // cell (the handle is a loaded payload, not the
                // cell).
                let cell_arg = if handle.handle_is_cell {
                    resolved.clone()
                } else {
                    handle.cell_reg.clone()
                };
                self.ins(&format!(
                    "EXPAND {} {}",
                    handle.kind.release_macro_name(),
                    cell_arg
                ));
            }
            if plan.consume_handle_value {
                self.ins(&format!("!{resolved}"));
            }
            if let Some(temp) = &handle.cell_release_temp {
                if plan.release_owner_temps {
                    let temp = temp.clone();
                    self.emit_release(&temp)?;
                }
            }
            return Ok(());
        }
        // codegen.zig:1186 `mutex_guard_handles` probe: releasing
        // a live guard unlocks its mutex, retires the guard
        // register, and restores the receiver for its own
        // scope-exit release. (Zig: mutex/rwlock/file/metadata
        // handle families — only the mutex and RefCell families
        // are populated in this slice; RwLock/file probes belong
        // at their own Zig positions when those families land.)
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
        // codegen.zig:1064 `rwlock_guard_handles` probe:
        // releasing a live read/write guard releases the lock
        // (READ or WRITE spelling by guard kind), retires the
        // guard register, and restores the receiver.
        if let Some(handle) = self.rwlock_guard_handles.remove(&resolved) {
            let macro_name = if handle.is_write {
                "RWLOCK_RELEASE_WRITE"
            } else {
                "RWLOCK_RELEASE_READ"
            };
            self.ins(&format!("EXPAND {macro_name} {}", handle.lock_reg));
            self.release_temporary_handle_register(&handle.lock_reg)?;
            self.consumed_bindings.insert(resolved.clone());
            self.ins(&format!("!{resolved}"));
            self.restore_rwlock_receiver_after_guard_release(&handle.lock_reg);
            return Ok(());
        }
        // codegen.zig:1084 `rwlock_lock_results` probe: a lock
        // Result dropped without `unwrap` releases the same way.
        if let Some(handle) = self.rwlock_lock_results.remove(&resolved) {
            let macro_name = if handle.is_write {
                "RWLOCK_RELEASE_WRITE"
            } else {
                "RWLOCK_RELEASE_READ"
            };
            self.ins(&format!("EXPAND {macro_name} {}", handle.lock_reg));
            self.release_temporary_handle_register(&handle.lock_reg)?;
            self.consumed_bindings.insert(resolved.clone());
            self.ins(&format!("!{resolved}"));
            self.restore_rwlock_receiver_after_guard_release(&handle.lock_reg);
            return Ok(());
        }
        // (Zig: file/metadata handle families — no such bindings
        // exist in the slice.)
        // codegen.zig:1342: releasing a cell register first
        // settles every live borrow handle taken from it (the
        // borrow must end before the cell's own release retires
        // the storage it points into). The handle being released
        // itself is skipped — its own probe above settled it.
        {
            let mut handles_to_release: Vec<String> = Vec::new();
            for (handle_name, handle) in &self.refcell_borrow_handles {
                if sla_lowering::refcell::plan_refcell_handle_cell_release(
                    handle.cell_reg == resolved,
                    handle_name == &resolved,
                ) {
                    handles_to_release.push(handle_name.clone());
                }
            }
            for handle_name in handles_to_release {
                self.emit_release(&handle_name)?;
            }
        }
        if resolved.starts_with('&') || resolved.starts_with('^') {
            return Ok(());
        }
        if self.stack_alloc_bindings.contains(&resolved) {
            // codegen.zig:1377: a stack slot may carry a RefCell
            // borrow in its companion slot; settle it as part of
            // the slot's release.
            if self.release_result_slot_refcell_handle(&resolved, true)? {
                self.consumed_bindings.insert(resolved.clone());
            }
            return Ok(());
        }
        if self.global_const_bindings.contains(&resolved) {
            return Ok(());
        }
        if self.consumed_bindings.contains(&resolved) {
            return Ok(());
        }
        if self.whole_moved_bindings.contains(&resolved) {
            // The binding's whole value was moved into an
            // aggregate literal (see `whole_moved_bindings`):
            // its fields are the aggregate's now, so the drop
            // cascade must not run — retire only the husk
            // register. Without this, `return (q, idx)` in
            // sa_std/slot_pool.sla freed the returned pool's
            // Vec fields at the return cleanups (use-after-free
            // in every later pool operation).
            self.consumed_bindings.insert(resolved.clone());
            self.ins(&format!("!{resolved}"));
            return Ok(());
        }
        // (Zig: mpsc sender/receiver, string-buffer and collection
        // binding families emit EXPAND *_FREE — unpopulated here.)
        // DEF-ARC-DROP (F1) plan dispatch (codegen.zig emitRelease's
        // plan switch; drop.rs): a named binding with a registered
        // type drops by plan before the generic `!` tail — struct
        // fields deep-drop (the tail then frees the husk),
        // Vec/GrowVec elements deep-drop and the container frees,
        // Arc/Rc/Box run the smart-pointer drop on a private copy
        // and settle WITHOUT the tail (see the arm below). Borrow
        // parameters never own their value; the struct-update site
        // suppresses the dispatch while settling its source
        // register (its fields were raw-moved or compensated
        // field-by-field at the site).
        if !self.plan_drop_suppressed && !self.borrow_param_bindings.contains(&resolved) {
            let binding_ty = self
                .local_binding_type_for_name(name)
                .or_else(|| self.local_binding_type_for_name(&resolved));
            if let Some(binding_ty) = binding_ty {
                // RUST-STAGE10-DROP-TAINT (B1): a tainted binding
                // aliases storage owned by someone else. A tainted
                // DeepStruct with a FRESH husk (call result) falls
                // through to the normal arm — its field drops are
                // already suppressed through update_moved_fields,
                // and the `!` tail frees the fresh husk. Every
                // other tainted deep value is forgotten: a tainted
                // DeepStruct with an ALIASED husk (place read)
                // must not free the source's block, and a tainted
                // DeepVec/Buffer container IS the aliased storage.
                if let Some(form) = self.tainted_bindings.get(&resolved).copied() {
                    let plan = self.plan_value_drop(&binding_ty);
                    let forget = match (&plan, form) {
                        (
                            crate::drop::DropPlan::DeepStruct(_),
                            crate::drop::TaintForm::AliasedHusk,
                        ) => true,
                        (crate::drop::DropPlan::DeepVec(_), _) => true,
                        (crate::drop::DropPlan::DeepGrowVec(_), _) => true,
                        (crate::drop::DropPlan::Buffer, _) => true,
                        (crate::drop::DropPlan::BufferGrowVec, _) => true,
                        _ => false,
                    };
                    if forget {
                        self.ins(&format!("^{resolved}"));
                        self.consumed_bindings.insert(resolved.clone());
                        return Ok(());
                    }
                }
                match self.plan_value_drop(&binding_ty) {
                    crate::drop::DropPlan::DeepStruct(fields) => {
                        self.emit_struct_field_drops(
                            &resolved,
                            &binding_ty,
                            &fields,
                            Some(name),
                        )?;
                    }
                    crate::drop::DropPlan::DeepVec(elem_plan) => {
                        self.emit_container_element_drops(
                            &resolved,
                            &binding_ty,
                            &elem_plan,
                            "Vec_ptr",
                            "Vec_len",
                        )?;
                        // Task 137 (P1): a stack-allocated Vec header
                        // frees only the buffer — the header itself is
                        // a stack slot, not a heap block (VEC_FREE
                        // would free the stack slot).
                        if self.stack_header_bindings.contains(&resolved) {
                            self.ins(&format!("EXPAND VEC_FREE_BUF {resolved}"));
                        } else {
                            self.ins(&format!("EXPAND VEC_FREE {resolved}"));
                        }
                        self.consumed_bindings.insert(resolved.clone());
                        return Ok(());
                    }
                    crate::drop::DropPlan::DeepGrowVec(elem_plan) => {
                        self.emit_container_element_drops(
                            &resolved,
                            &binding_ty,
                            &elem_plan,
                            "GrowVec_ptr",
                            "GrowVec_len",
                        )?;
                        self.ins(&format!("EXPAND GROWVEC_FREE {resolved}"));
                        self.consumed_bindings.insert(resolved.clone());
                        return Ok(());
                    }
                    // DEF-RUST-VEC-SCOPE-EXIT-NO-FREE (task 72): a
                    // Buffer-plan binding (Vec<T> with primitive T,
                    // or String) owns a heap buffer that the generic
                    // `!` tail does not free -- the tail releases the
                    // register only, so the backing store leaked once
                    // per scope exit (churnops retained ~200 MB). The
                    // Zig mirror frees these through its
                    // vec_buf_bindings map; the Rust port dropped the
                    // maps, and this dispatch had no Buffer arm, so
                    // the binding fell through to `!`. Mirror the
                    // DeepVec arm: free the buffer and retire the
                    // register without the `!` tail (VEC_FREE's
                    // expansion consumes the register itself via
                    // `^`; STRING_BUF_FREE expands to the same
                    // macro, so String is covered by this arm too).
                    crate::drop::DropPlan::Buffer => {
                        // Task 137 (P1): a stack-allocated Vec header
                        // frees only the buffer — the header itself is
                        // a stack slot, not a heap block. (Buffer also
                        // covers String, which can never be in
                        // stack_header_bindings — only Vec bindings
                        // are recorded there — so this branch is
                        // Vec-only in practice.)
                        if self.stack_header_bindings.contains(&resolved) {
                            self.ins(&format!("EXPAND VEC_FREE_BUF {resolved}"));
                        } else {
                            self.ins(&format!("EXPAND VEC_FREE {resolved}"));
                        }
                        self.consumed_bindings.insert(resolved.clone());
                        return Ok(());
                    }
                    crate::drop::DropPlan::BufferGrowVec => {
                        self.ins(&format!("EXPAND GROWVEC_FREE {resolved}"));
                        self.consumed_bindings.insert(resolved.clone());
                        return Ok(());
                    }
                    plan @ (crate::drop::DropPlan::ArcPtr(_)
                    | crate::drop::DropPlan::RcPtr(_)
                    | crate::drop::DropPlan::BoxPtr(_)) => {
                        self.emit_smart_pointer_drop_text(&resolved, &binding_ty, &plan, false)?;
                        // No generic `!` tail for a smart pointer:
                        // the register owns the shared box, and a
                        // bare `!` on the owning register frees the
                        // block outright, bypassing the count the
                        // drop sequence just maintained (verified
                        // empirically against the pinned host:
                        // keeping the tail corrupts the strong
                        // count of every surviving handle). The
                        // value was consumed by the drop sequence
                        // (through its private copy), so the
                        // register is retired with the forget
                        // marker `^name` — the same consume the
                        // emitter uses for moved-out values — which
                        // also keeps a following rebinding of the
                        // name (assignment overwrite) from tripping
                        // RegisterRedefinition.
                        self.ins(&format!("^{resolved}"));
                        self.consumed_bindings.insert(resolved.clone());
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
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
        // DEF-P1-REV-TEXT-EMIT (codegen.zig:2082): a held
        // iterator-chain binding owns nothing and materializes
        // no register; releasing it would emit `!name` for a
        // register that was never declared (trap 1007). Skip
        // it — the underlying collection is released by its
        // own cleanup (iter_window.rs, LINE-FITER-TEXT-PORT).
        if self.iter_chain_bindings.contains_key(name) {
            return Ok(());
        }
        let resolved = self.resolve_binding_name(name);
        if self.iter_chain_bindings.contains_key(&resolved) {
            return Ok(());
        }
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
                self.emit_function_result_cleanup(&name, tail_expr)?;
            }
        }
        Ok(())
    }

    /// Act on one function-result cleanup: release the binding when
    /// the plan says so; retire only its husk when the plan
    /// transfers a binding moved wholesale into the returned
    /// composite; skip it when the value itself is returned (a
    /// bare-identifier transfer — the register leaves with the
    /// return value, so there is no husk to retire).
    ///
    /// The transfer arm is the L2R2 Fix A follow-up (R546
    /// A_retire_marking). Fix A (d7e38e0a) returned TransferResult
    /// for the moved-into-composite shape and then skipped the
    /// release outright, so the source register stayed Active at
    /// the exit boundary (MemoryLeak on reg1_icmp and on the two
    /// head-green controls it regressed). `emit_release` retires
    /// only the husk for whole-moved bindings — no field drops,
    /// so the storage the caller receives is never freed — and is
    /// a no-op when the binding was already consumed (the TEXT35
    /// root lane). A bare-identifier transfer never lands here:
    /// its register IS the returned value, and it is not
    /// whole-moved.
    pub(crate) fn emit_function_result_cleanup(
        &mut self,
        name: &str,
        result_expr: &Node,
    ) -> Result<()> {
        let action = self.plan_function_result_cleanup(name, result_expr)?;
        if action.releases_value() {
            return self.emit_release(name);
        }
        let resolved = self.resolve_binding_name(name);
        if self.whole_moved_bindings.contains(&resolved) {
            return self.emit_release(name);
        }
        Ok(())
    }

    /// codegen.zig:1393 `planFunctionResultCleanup`: the lowering
    /// tail-cleanup plan, widened to a transfer when the cleanup name
    /// is the result's root and the result type is a non-Copy,
    /// non-borrow-like value (the value itself is being returned) —
    /// but only when generating the result actually consumed the root
    /// (see the TEXT35-OWNERSHIP guard below).
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
                        // TEXT35-OWNERSHIP (DEF-TEXT-EXTRACT-ROOT-TRANSFER,
                        // codegen.zig `planFunctionResultCleanupResolved`):
                        // a field or index extraction result transfers the
                        // root binding only when generating the result
                        // actually consumed it. A plain extraction copies
                        // the element/field out (load into a fresh
                        // register), leaving the root register live: it
                        // must take the base plan's normal release or it
                        // leaks at the exit boundary
                        // (`return self.items[index]`). This lane keeps no
                        // separate store-moved table — every move/store
                        // recording lands in `consumed_bindings`
                        // (`mark_consumed_binding`), so the consumed probe
                        // is the lane-faithful form of the Zig
                        // consumed/store-moved pair.
                        let resolved = self.resolve_binding_name(cleanup_name);
                        if !self.consumed_bindings.contains(&resolved) {
                            return Ok(base_plan);
                        }
                        return Ok(FunctionTailCleanupAction::TransferResult);
                    }
                }
            }
        }
        // SPRINT-100 L2R2 Fix A follow-up (R546 A_retire_marking):
        // a composite result that MOVES the cleanup binding into
        // the returned aggregate transfers it exactly like a
        // bare-identifier result. The base plan only recognises an
        // identifier / ^identifier RESULT, so a binding stored into
        // a returned tuple/struct literal or an Option/Result
        // constructor payload was released at the tail anyway —
        // freeing the very storage the caller receives
        // (icmp_parse_message: the payload Vec freed after
        // RESULT_NEW_OK; tri_dispatch / move_bag_advance: the Vec
        // deep-dropped / freed after the tuple store; pod_register:
        // the registry Vec freed after the tuple store). Only
        // bare-identifier / ^identifier constituents count: a
        // field/index constituent is an extraction, governed by
        // the root rule above, and a general call argument may
        // have been cloned by the call-arg planners.
        //
        // The transfer alone is NOT the fix (Fix A d7e38e0a
        // stopped here: it suppressed the tail free but never
        // retired the source register, so the verifier flagged it
        // Active at the exit boundary — MemoryLeak, plus the two
        // head-green regressions). The binding is therefore ALSO
        // recorded whole-moved, so the tail emitters retire only
        // its husk (`emit_function_result_cleanup`) instead of
        // skipping its release outright. Tuple/struct/enum
        // constituents are usually already recorded by generation
        // (`mark_binding_moved_into_aggregate`); the constructor
        // payload leg (Some/Ok/Err via call.rs EXPAND forms) is
        // NOT — this insert covers it uniformly.
        if result_moves_binding(result_expr, cleanup_name) {
            let binding_ty = self
                .local_binding_type_for_name(cleanup_name)
                .or_else(|| {
                    self.local_binding_type_for_name(
                        &self.resolve_binding_name(cleanup_name),
                    )
                });
            if let Some(binding_ty) = binding_ty {
                if !self.type_is_copy_value(&binding_ty)
                    && !is_borrow_like_type(&binding_ty)
                {
                    let resolved = self.resolve_binding_name(cleanup_name);
                    self.whole_moved_bindings.insert(resolved);
                    return Ok(FunctionTailCleanupAction::TransferResult);
                }
            }
        }
        Ok(base_plan)
    }

    /// Record a whole-binding move into an aggregate literal:
    /// the element/field value is a bare identifier (or an
    /// explicit move of one) of a non-Copy, non-borrow-like
    /// type, so the checker consumed the binding into the
    /// aggregate. Mirrors the ownership fact behind Zig's
    /// `markMovedExprBinding` / cell model (a moved-out local's
    /// own release is trivial); this backend has no cell, so
    /// the transfer is recorded on the binding itself and
    /// `emit_release` retires only the husk.
    pub(crate) fn mark_binding_moved_into_aggregate(
        &mut self,
        expr: &Node,
        ty: &Type,
    ) {
        let name = match expr {
            Node::Identifier(n) => n.as_str(),
            Node::MoveExpr(m) => match &*m.expr {
                Node::Identifier(n) => n.as_str(),
                _ => return,
            },
            _ => return,
        };
        if self.type_is_copy_value(ty) || is_borrow_like_type(ty) {
            return;
        }
        let resolved = self.resolve_binding_name(name);
        self.whole_moved_bindings.insert(resolved);
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
        // codegen.zig:2776: inside a user-macro inline, the
        // recorded macro-call try cleanup list replaces the
        // node's own branch-scoped list.
        //
        // DEF-RUST-REFCELL-TRYPROP-BORROW-LEAK (task 22): the
        // try's None/Err arm is a terminating branch — its
        // cleanup must not disturb the surviving (Some/Ok)
        // path's state. `emit_release` destructively removes a
        // released borrow handle from `refcell_borrow_handles`,
        // and the cleanup list only snapshots/restores the
        // consumed state, so without restoring the handle table
        // the success path's later cleanup degrades to a plain
        // `!handle` and the shared borrow count leaks
        // (panic 30935). Snapshot and restore the handle table
        // around the branch cleanup, exactly as `gen_if_expr` /
        // `gen_let_else_stmt` restore the three tables around a
        // branch side.
        let saved_handles = self.refcell_borrow_handles.clone();
        if let Some(names) = self.active_macro_try_cleanup.clone() {
            self.emit_branch_scoped_cleanup_list(&names)?;
        } else if let Some(list) = self.check.cleanups(node) {
            let names: Vec<String> = list.to_vec();
            self.emit_branch_scoped_cleanup_list(&names)?;
        }
        self.refcell_borrow_handles = saved_handles;
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
