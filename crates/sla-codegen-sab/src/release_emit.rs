//! E7 — release emission, mirroring sab_codegen.zig lines
//! 4240-4760 in the reference tree (/content/sa_all, HEAD
//! 2cc12ea8): releaseExprResultIfNeeded and its stored/copied
//! siblings, the RefCell borrow releases, emitRelease /
//! emitMove / emitForgetMovedValue, markConsumed, the
//! future-state transfers and readiness bookkeeping
//! (SAB-D11). (Decomposition lines 3578-3847, older tree.)
//!
//! Boundaries: guard and RefCell releases expand std macros
//! through the F group's fragment machine, consumed here
//! through MacroFragmentEmitter whose default
//! NoMacroFragments fails loudly (wiring need, E REPORT).
//! emitRelease's Zig bookkeeping over parallel hash sets
//! maps onto the frame's SAB-D09 classification (RegState
//! variants; field_handle_view_regs / stack_alloc_emitted
//! stay payload sets). No error is swallowed (SAB-D03);
//! the future-readiness orelse-unknown query default
//! (SAB-D11) is reproduced in future_readiness_for_state,
//! registered, not silently fixed.

use std::collections::HashSet;

use sla_ast::{Node, Param, Primitive, Type};
use sla_lowering::{
    abi_passes_as_pointer, by_value_raw_pointer_param, expr_result_needs_release,
    is_borrow_like_type, shared_abi_param_cap, stored_value_moves_identifier, AbiCapKind,
    CheckInfo,
};

use crate::captures::{is_local_reg, stack_local};
use crate::copy_facts::CopyFacts;
use crate::error::{Error, Result};
use crate::ir::{CapPrefix, InstKind, Operand, PrimType};
use crate::release_plans::{
    plan_borrow_address_temp_release, plan_refcell_handle_cell_release, plan_refcell_handle_release,
    refcell_borrow_release_macro_name,
};
use crate::release_prim::{
    append_inst, emit_consumed_marker, emit_store, is_released, make_inst, mark_released,
    reg_name,
};
use crate::state::{CodegenState, RegState};
use crate::types::{FutureReadiness, MutexGuardValue, RefCellBorrowValue, RwLockGuardValue};

/// The F-group std-macro fragment machine as E7 consumes it
/// (Zig `emitStdMacroFragment` /
/// `emitStdMacroFragmentWithLiteralArgs`, sab_codegen.zig
/// lines 6299–6322 in the reference tree).
pub trait MacroFragmentEmitter {
    fn emit_std_macro_fragment(
        &mut self,
        import_path: &str,
        macro_name: &str,
        args: &[String],
    ) -> Result<()>;
    fn emit_std_macro_fragment_with_literal_args(
        &mut self,
        import_path: &str,
        macro_name: &str,
        args: &[String],
        literal_args: &[bool],
    ) -> Result<()>;
}

/// Loud default fragment emitter: the fragment machine is not
/// wired into this build, so any macro-shaped release fails
/// with its coordinates instead of degrading.
pub struct NoMacroFragments;

impl MacroFragmentEmitter for NoMacroFragments {
    fn emit_std_macro_fragment(
        &mut self,
        import_path: &str,
        macro_name: &str,
        _args: &[String],
    ) -> Result<()> {
        Err(Error::UnsupportedSabDirectFeature(format!(
            "emitStdMacroFragment {import_path}::{macro_name} [group E7 release_emit; F-group fragment machine not wired]"
        )))
    }
    fn emit_std_macro_fragment_with_literal_args(
        &mut self,
        import_path: &str,
        macro_name: &str,
        _args: &[String],
        _literal_args: &[bool],
    ) -> Result<()> {
        Err(Error::UnsupportedSabDirectFeature(format!(
            "emitStdMacroFragmentWithLiteralArgs {import_path}::{macro_name} [group E7 release_emit; F-group fragment machine not wired]"
        )))
    }
}

/// Zig `markConsumed`: bookkeeping only — the register joins
/// the released set so later cleanups stay silent.
pub fn mark_consumed(state: &mut CodegenState, reg: u32) {
    mark_released(state, reg);
}

/// Zig `emitMove`: a raw `move_` instruction plus released
/// bookkeeping; a no-op for already-released registers.
pub fn emit_move(state: &mut CodegenState, reg: u32) {
    if is_released(state, reg) {
        return;
    }
    let mut inst = make_inst(state, InstKind::Move);
    inst.operands[0] = Operand::Reg(reg);
    append_inst(state, inst);
    mark_released(state, reg);
}

/// Zig `emitAssignmentMove`: the same instruction as
/// [`emit_move`] but unconditional (no released check).
pub fn emit_assignment_move(state: &mut CodegenState, reg: u32) {
    let mut inst = make_inst(state, InstKind::Move);
    inst.operands[0] = Operand::Reg(reg);
    append_inst(state, inst);
    mark_released(state, reg);
}

/// Zig `emitBranchRelease`: a raw release instruction with NO
/// released-set bookkeeping (branch paths account separately).
pub fn emit_branch_release(state: &mut CodegenState, reg: u32) {
    let mut inst = make_inst(state, InstKind::Release);
    inst.operands[0] = Operand::Reg(reg);
    append_inst(state, inst);
}

/// Zig `emitBranchMove`: a raw `move_` with no bookkeeping.
pub fn emit_branch_move(state: &mut CodegenState, reg: u32) {
    let mut inst = make_inst(state, InstKind::Move);
    inst.operands[0] = Operand::Reg(reg);
    append_inst(state, inst);
}

/// Zig `emitMutexGuardRelease` (line 16000).
pub fn emit_mutex_guard_release(state: &mut CodegenState, frags: &mut dyn MacroFragmentEmitter, handle: MutexGuardValue) -> Result<()> {
    let name = reg_name(state, handle.mutex_reg);
    frags.emit_std_macro_fragment("sa_std/sync/mutex.sa", "MUTEX_UNLOCK", &[name])
}

/// Zig `emitRwLockGuardRelease` (line 16006).
pub fn emit_rwlock_guard_release(state: &mut CodegenState, frags: &mut dyn MacroFragmentEmitter, handle: RwLockGuardValue) -> Result<()> {
    let name = reg_name(state, handle.lock_reg);
    if handle.is_write {
        frags.emit_std_macro_fragment("sa_std/sync/rwlock.sa", "RWLOCK_RELEASE_WRITE", &[name])
    } else {
        frags.emit_std_macro_fragment("sa_std/sync/rwlock.sa", "RWLOCK_RELEASE_READ", &[name])
    }
}

/// Zig `emitRefCellBorrowRelease`: the dynamic borrow is
/// released through the RefCell macro; owner temps follow
/// through the non-local temp release.
pub fn emit_refcell_borrow_release(state: &mut CodegenState, frags: &mut dyn MacroFragmentEmitter, handle: RefCellBorrowValue) -> Result<()> {
    let plan = plan_refcell_handle_release(!handle.release_regs.is_empty());
    if plan.release_dynamic_borrow {
        let name = reg_name(state, handle.cell_reg);
        frags.emit_std_macro_fragment(
            "sa_std/core/refcell.sa",
            refcell_borrow_release_macro_name(handle.kind),
            &[name],
        )?;
    }
    if plan.release_owner_temps {
        release_non_local_temps(state, frags, &handle.release_regs)?;
    }
    Ok(())
}

/// Zig `emitRefCellBorrowBranchRelease`: the branch form —
/// owner temps go through the metadata-aware branch release
/// under the caller's `seen` set.
pub fn emit_refcell_borrow_branch_release(state: &mut CodegenState, frags: &mut dyn MacroFragmentEmitter, handle: RefCellBorrowValue, seen: &mut HashSet<u32>) -> Result<()> {
    let plan = plan_refcell_handle_release(!handle.release_regs.is_empty());
    if plan.release_dynamic_borrow {
        let name = reg_name(state, handle.cell_reg);
        frags.emit_std_macro_fragment(
            "sa_std/core/refcell.sa",
            refcell_borrow_release_macro_name(handle.kind),
            &[name],
        )?;
    }
    if plan.release_owner_temps {
        for reg in &handle.release_regs {
            if !is_local_reg(state, *reg) {
                emit_branch_release_with_metadata(state, frags, *reg, seen)?;
            }
        }
    }
    Ok(())
}

/// Zig `emitRefCellBorrowReleasesForCell`: release every live
/// RefCell handle whose cell is `cell_reg` (handles on the cell
/// itself are skipped by the plan — a handle never releases
/// itself through its own cell).
pub fn emit_refcell_borrow_releases_for_cell(state: &mut CodegenState, frags: &mut dyn MacroFragmentEmitter, cell_reg: u32) -> Result<()> {
    let mut handles: Vec<u32> = Vec::new();
    // Zig iterates the hash map here; the collection is only a
    // work list for releases whose own bookkeeping is per-reg,
    // but iteration order decides instruction order. Zig's
    // order is hash order (non-deterministic); the port emits
    // in ascending register order — a deliberate, registered
    // deviation under the hash-order discipline (SAB-D101 in
    // the E-group REPORT), never Zig's pointer-hash order.
    let mut keys: Vec<u32> = state.frame.refcell_borrow_values.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let value = &state.frame.refcell_borrow_values[&key];
        let action = plan_refcell_handle_cell_release(value.cell_reg == cell_reg, key == cell_reg);
        if action.should_release() {
            handles.push(key);
        }
    }
    for handle_reg in handles {
        if !is_released(state, handle_reg) {
            emit_release(state, frags, handle_reg)?;
        }
    }
    Ok(())
}

/// Zig `emitBranchReleaseWithMetadata`: branch-scoped release
/// that first releases RefCell handles living on the register,
/// then the register's own RefCell handle, honors non-owning
/// views, and finally restores/releases borrow-address temps.
pub fn emit_branch_release_with_metadata(
    state: &mut CodegenState,
    frags: &mut dyn MacroFragmentEmitter,
    reg: u32,
    seen: &mut HashSet<u32>,
) -> Result<()> {
    if seen.contains(&reg) {
        return Ok(());
    }
    seen.insert(reg);
    if is_released(state, reg) {
        return Ok(());
    }
    let mut handles: Vec<u32> = Vec::new();
    let mut keys: Vec<u32> = state.frame.refcell_borrow_values.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let value = &state.frame.refcell_borrow_values[&key];
        let action = plan_refcell_handle_cell_release(value.cell_reg == reg, key == reg);
        if action.should_release() {
            handles.push(key);
        }
    }
    for handle_reg in handles {
        emit_branch_release_with_metadata(state, frags, handle_reg, seen)?;
    }
    if let Some(handle) = state.frame.refcell_borrow_values.get(&reg).cloned() {
        emit_refcell_borrow_branch_release(state, frags, handle, seen)?;
    }
    if state.frame.reg_state(reg) == RegState::NonOwning {
        return Ok(());
    }
    let borrow_plan = plan_borrow_address_temp_release(state.frame.borrow_address_temps.contains_key(&reg));
    emit_branch_release(state, reg);
    if borrow_plan.release_source_temps {
        if let Some(temp) = state.frame.borrow_address_temps.get(&reg).cloned() {
            if let Some(restore_slot) = temp.restore_slot {
                let restore_value = temp.restore_value.ok_or_else(|| {
                    Error::UnsupportedSabDirectFeature(
                        "borrow address temp has restore slot but no restore value [group E7 release_emit]"
                            .to_string(),
                    )
                })?;
                emit_store(state, restore_slot, 0, restore_value, PrimType::Ptr);
            }
            for t in &temp.release_regs {
                if temp.restore_value == Some(*t) {
                    continue;
                }
                emit_branch_release_with_metadata(state, frags, *t, seen)?;
            }
        }
    }
    Ok(())
}

/// Zig `emitRelease`: the full single-register release —
/// RefCell handles, RefCell/guard payloads, stack slots
/// (bookkeeping only), field views, non-owning views, and
/// finally a plain release plus borrow-temp disposal.
pub fn emit_release(state: &mut CodegenState, frags: &mut dyn MacroFragmentEmitter, reg: u32) -> Result<()> {
    if is_released(state, reg) {
        return Ok(());
    }
    emit_refcell_borrow_releases_for_cell(state, frags, reg)?;
    if let Some(handle) = state.frame.refcell_borrow_values.remove(&reg) {
        emit_refcell_borrow_release(state, frags, handle)?;
    }
    if let Some(handle) = state.frame.mutex_guard_values.remove(&reg) {
        emit_mutex_guard_release(state, frags, handle)?;
    }
    if let Some(handle) = state.frame.rwlock_guard_values.remove(&reg) {
        emit_rwlock_guard_release(state, frags, handle)?;
    }
    if state.frame.reg_state(reg) == RegState::StackAlloc || state.frame.stack_alloc_emitted.contains(&reg) {
        // Stack slots die with their scope: emitting release or
        // move_ traps StackEscape in the verifier. Mark released
        // so scope-end accounting stays silent (Zig comment).
        if state.frame.reg_state(reg) == RegState::NonOwning {
            state.frame.reg_states.remove(&reg);
        }
        state.frame.future_state_vtables.remove(&reg);
        state.frame.future_readiness.remove(&reg);
        mark_released(state, reg);
        return Ok(());
    }
    if state.frame.field_handle_view_regs.remove(&reg) {
        // DEF-KB-1: handle-typed field views need a real release
        // (move_ is a verifier no-op for raw-pointer registers).
        if state.frame.reg_state(reg) == RegState::NonOwning {
            state.frame.reg_states.remove(&reg);
        }
        state.frame.future_state_vtables.remove(&reg);
        state.frame.future_readiness.remove(&reg);
        let mut inst = make_inst(state, InstKind::Release);
        inst.operands[0] = Operand::Reg(reg);
        append_inst(state, inst);
        mark_released(state, reg);
        return Ok(());
    }
    if state.frame.reg_state(reg) == RegState::NonOwning {
        let mut inst = make_inst(state, InstKind::Move);
        inst.operands[0] = Operand::Reg(reg);
        append_inst(state, inst);
        mark_released(state, reg);
        return Ok(());
    }
    let borrow_plan = plan_borrow_address_temp_release(state.frame.borrow_address_temps.contains_key(&reg));
    state.frame.future_state_vtables.remove(&reg);
    state.frame.future_readiness.remove(&reg);
    let mut inst = make_inst(state, InstKind::Release);
    inst.operands[0] = Operand::Reg(reg);
    append_inst(state, inst);
    mark_released(state, reg);
    if borrow_plan.release_source_temps {
        if let Some(temp) = state.frame.borrow_address_temps.remove(&reg) {
            if let Some(restore_slot) = temp.restore_slot {
                let restore_value = temp.restore_value.ok_or_else(|| {
                    Error::UnsupportedSabDirectFeature(
                        "borrow address temp has restore slot but no restore value [group E7 release_emit]"
                            .to_string(),
                    )
                })?;
                emit_store(state, restore_slot, 0, restore_value, PrimType::Ptr);
                mark_consumed(state, restore_value);
            }
            for t in temp.release_regs.clone() {
                emit_release(state, frags, t)?;
            }
        }
    }
    Ok(())
}

/// Zig `emitForgetMovedValue`: retire a value moved into a
/// future state/wrapper — stack slots by bookkeeping only,
/// everything else via `move_` (via [`emit_move`]).
pub fn emit_forget_moved_value(state: &mut CodegenState, reg: u32) {
    if is_released(state, reg) {
        return;
    }
    if state.frame.reg_state(reg) == RegState::StackAlloc || state.frame.stack_alloc_emitted.contains(&reg) {
        if state.frame.reg_state(reg) == RegState::NonOwning {
            state.frame.reg_states.remove(&reg);
        }
        state.frame.future_state_vtables.remove(&reg);
        state.frame.future_readiness.remove(&reg);
        mark_released(state, reg);
        return;
    }
    state.frame.future_state_vtables.remove(&reg);
    state.frame.future_readiness.remove(&reg);
    emit_move(state, reg);
}

/// Zig `releaseNonLocalTemps`: release every listed register
/// that is not a local.
pub fn release_non_local_temps(state: &mut CodegenState, frags: &mut dyn MacroFragmentEmitter, regs: &[u32]) -> Result<()> {
    for reg in regs {
        if !is_local_reg(state, *reg) {
            emit_release(state, frags, *reg)?;
        }
    }
    Ok(())
}

/// Zig `typeIsPointerScalarValue` (lowering_rules.zig:6589).
pub fn type_is_pointer_scalar_value(ty: &Type) -> bool {
    match ty {
        Type::Primitive(p) => *p == Primitive::RawPtr,
        Type::Pointer(_) => true,
        _ => false,
    }
}

/// Zig `isHeapHandleType` (lowering_rules.zig:1323).
pub fn is_heap_handle_type(ty: &Type) -> bool {
    matches!(ty, Type::UserDefined(_) | Type::Tuple(_) | Type::Array(_))
}

/// Zig `releaseExprResultIfNeeded`.
pub fn release_expr_result_if_needed(state: &mut CodegenState, frags: &mut dyn MacroFragmentEmitter, check: &dyn CheckInfo, expr: &Node, reg: u32) -> Result<()> {
    if let Node::Identifier(name) = expr {
        if stack_local(state, name).is_some() {
            if !is_local_reg(state, reg) {
                emit_release(state, frags, reg)?;
            }
            return Ok(());
        }
    }
    if !expr_result_needs_release(expr) {
        return Ok(());
    }
    if !is_local_reg(state, reg) {
        // An unconsumed owned String/Vec temporary owns its
        // buffer: drop it deeply instead of leaking the buffer
        // behind a bare register release.
        let ty = check.expr_type(expr);
        if crate::release_locals::owned_buffer_drop_type(ty) {
            let ty = ty.cloned();
            crate::release_locals::emit_owned_buffer_free(state, reg, ty.as_ref())?;
        } else {
            emit_release(state, frags, reg)?;
        }
    }
    Ok(())
}

/// Zig `releaseStoredExprResultIfNeeded` (DEF-ACL ownership
/// transfer for heap-handle stores).
pub fn release_stored_expr_result_if_needed(
    state: &mut CodegenState,
    frags: &mut dyn MacroFragmentEmitter,
    expr: &Node,
    reg: u32,
    stored_ty: &Type,
) -> Result<()> {
    if let Node::Identifier(name) = expr {
        if stack_local(state, name).is_some() {
            if !is_local_reg(state, reg) {
                if is_heap_handle_type(stored_ty) {
                    emit_consumed_marker(state, reg);
                    mark_consumed(state, reg);
                    return Ok(());
                }
                if type_is_pointer_scalar_value(stored_ty) {
                    crate::release_meta::mark_non_owning_reg(state, reg);
                }
                emit_release(state, frags, reg)?;
            }
            return Ok(());
        }
    }
    if !expr_result_needs_release(expr) {
        return Ok(());
    }
    if !is_local_reg(state, reg) {
        if is_heap_handle_type(stored_ty) {
            emit_consumed_marker(state, reg);
            mark_consumed(state, reg);
            return Ok(());
        }
        if type_is_pointer_scalar_value(stored_ty) {
            crate::release_meta::mark_non_owning_reg(state, reg);
        }
        emit_release(state, frags, reg)?;
    }
    Ok(())
}

/// Zig `releaseCopiedFieldValue` (DEF-ACL).
pub fn release_copied_field_value(
    state: &mut CodegenState,
    frags: &mut dyn MacroFragmentEmitter,
    copied: u32,
    field_ty: &Type,
) -> Result<()> {
    if is_heap_handle_type(field_ty) {
        emit_consumed_marker(state, copied);
        mark_consumed(state, copied);
        return Ok(());
    }
    emit_release(state, frags, copied)
}

/// Zig `releaseCopiedArrayFieldSource` (the DEF-ACL exception:
/// a verbatim copy of the pre-DEF-ACL helper body — no
/// heap-handle transfer arm).
pub fn release_copied_array_field_source(
    state: &mut CodegenState,
    frags: &mut dyn MacroFragmentEmitter,
    expr: &Node,
    reg: u32,
    stored_ty: &Type,
) -> Result<()> {
    if let Node::Identifier(name) = expr {
        if stack_local(state, name).is_some() {
            if !is_local_reg(state, reg) {
                if type_is_pointer_scalar_value(stored_ty) {
                    crate::release_meta::mark_non_owning_reg(state, reg);
                }
                emit_release(state, frags, reg)?;
            }
            return Ok(());
        }
    }
    if !expr_result_needs_release(expr) {
        return Ok(());
    }
    if !is_local_reg(state, reg) {
        if type_is_pointer_scalar_value(stored_ty) {
            crate::release_meta::mark_non_owning_reg(state, reg);
        }
        emit_release(state, frags, reg)?;
    }
    Ok(())
}

/// Zig `paramCapability`: the shared ABI cap for a parameter,
/// collapsed to the SAB cap prefix (borrow/move survive,
/// everything else is by-value).
pub fn param_capability(check: &dyn CheckInfo, param: &Param) -> CapPrefix {
    let mut facts = CopyFacts::new(check);
    let cap = shared_abi_param_cap(
        param.is_borrow,
        param.is_move,
        matches!(*param.ty, Type::Borrow(_)),
        by_value_raw_pointer_param(param),
        facts.type_is_copy_value(&param.ty),
        is_borrow_like_type(&param.ty),
    );
    match cap {
        AbiCapKind::Borrow => CapPrefix::Borrow,
        AbiCapKind::Move => CapPrefix::Move,
        _ => CapPrefix::ByValue,
    }
}

/// Zig `markStoredValueMovedIfNeeded`.
pub fn mark_stored_value_moved_if_needed(
    state: &mut CodegenState,
    check: &dyn CheckInfo,
    value: &Node,
    value_ty: &Type,
) {
    let mut facts = CopyFacts::new(check);
    let is_copy = facts.type_is_copy_value(value_ty);
    if stored_value_moves_identifier(value, value_ty, is_copy).is_none() {
        return;
    }
    if let Node::Identifier(name) = value {
        if let Some(reg) = state.frame.local_reg(name) {
            mark_consumed(state, reg);
        }
    }
}

/// Zig `consumeStoredMoveValue`.
pub fn consume_stored_move_value(
    state: &mut CodegenState,
    check: &dyn CheckInfo,
    value: &Node,
    value_reg: u32,
    value_ty: &Type,
) {
    mark_stored_value_moved_if_needed(state, check, value, value_ty);
    if !matches!(value, Node::Identifier(_)) && expr_result_needs_release(value) {
        mark_consumed(state, value_reg);
    }
}

/// Zig `markLoadedFieldViewIfNeeded` (incl. the DEF-KB-1
/// field-handle-view registration).
pub fn mark_loaded_field_view_if_needed(
    state: &mut CodegenState,
    check: &dyn CheckInfo,
    reg: u32,
    field_ty: &Type,
) {
    let mut facts = CopyFacts::new(check);
    if facts.type_is_copy_value(field_ty) || is_borrow_like_type(field_ty) {
        return;
    }
    crate::release_meta::mark_non_owning_reg(state, reg);
    if abi_passes_as_pointer(field_ty) {
        state.frame.field_handle_view_regs.insert(reg);
    }
}

/// Zig `transferFutureStateVTable`.
pub fn transfer_future_state_vtable(state: &mut CodegenState, src: u32, dst: u32) {
    if let Some(vt) = state.frame.future_state_vtables.get(&src).cloned() {
        state.frame.future_state_vtables.insert(dst, vt);
        if src != dst {
            state.frame.future_state_vtables.remove(&src);
        }
    }
}

/// Zig `futureReadinessForState`: SAB-D11's `orelse .unknown`
/// query default, reproduced as registered.
pub fn future_readiness_for_state(state: &CodegenState, state_reg: u32) -> FutureReadiness {
    state
        .frame
        .future_readiness
        .get(&state_reg)
        .copied()
        .unwrap_or(FutureReadiness::Unknown)
}

/// Zig `recordFutureReadiness`: `Unknown` erases the entry.
pub fn record_future_readiness(state: &mut CodegenState, state_reg: u32, readiness: FutureReadiness) {
    if readiness == FutureReadiness::Unknown {
        state.frame.future_readiness.remove(&state_reg);
        return;
    }
    state.frame.future_readiness.insert(state_reg, readiness);
}

/// Zig `transferFutureReadiness`.
pub fn transfer_future_readiness(state: &mut CodegenState, src: u32, dst: u32) {
    if let Some(readiness) = state.frame.future_readiness.get(&src).copied() {
        record_future_readiness(state, dst, readiness);
        if src != dst {
            state.frame.future_readiness.remove(&src);
        }
        return;
    }
    state.frame.future_readiness.remove(&dst);
}

/// Zig `assignSourceNeedsCleanup`: dedicated-lifecycle
/// registers keep their scope-end cleanup.
pub fn assign_source_needs_cleanup(state: &CodegenState, src: u32) -> bool {
    if state.frame.reg_state(src) == RegState::NonOwning {
        return true;
    }
    if state.frame.reg_state(src) == RegState::StackAlloc {
        return true;
    }
    if state.frame.stack_alloc_emitted.contains(&src) {
        return true;
    }
    if state.frame.mutex_guard_values.contains_key(&src) {
        return true;
    }
    if state.frame.rwlock_guard_values.contains_key(&src) {
        return true;
    }
    if state.frame.refcell_borrow_values.contains_key(&src) {
        return true;
    }
    if state.frame.borrow_address_temps.contains_key(&src) {
        return true;
    }
    false
}

/// Zig `emitAssignReg` (full form): the builder's
/// `emit_assign_reg` is the instruction shape only; this is
/// the whole Zig body — object-pointer provenance propagation,
/// release-metadata transfer, source consumption, and the
/// DEF-KB-1 sibling field-view disposal.
pub fn emit_assign_reg(state: &mut CodegenState, frags: &mut dyn MacroFragmentEmitter, dst: u32, src: u32) -> Result<()> {
    if dst == src {
        return Ok(());
    }
    if crate::locals::reg_is_object_ptr(state, src) {
        crate::locals::mark_object_ptr_reg(state, dst);
    }
    state.builder.emit_assign_reg(dst, src);
    crate::release_meta::transfer_release_metadata(state, dst, src)?;
    if !is_local_reg(state, src) {
        mark_consumed(state, src);
    }
    if !assign_source_needs_cleanup(state, src) {
        mark_consumed(state, src);
    }
    if state.frame.field_handle_view_regs.contains(&src) {
        state.frame.reg_states.remove(&src);
        emit_release(state, frags, src)?;
    }
    Ok(())
}
