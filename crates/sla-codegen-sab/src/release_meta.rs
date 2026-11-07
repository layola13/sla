//! E4 — release metadata, mirroring `sab_codegen.zig` lines
//! 2953–3218 in the reference tree (`/content/sa_all`, HEAD
//! 2cc12ea8): the single-register release sets, borrow-address
//! temp remembering, RefCell/result-slot value-state transfers,
//! the result-slot RefCell companion machinery, and
//! `markNonOwningReg`. (Decomposition doc lines 2768–2953 in
//! the older tree.)
//!
//! The `plan*` decision functions live in Zig's
//! `lowering_rules.zig` / `control_flow_rules.zig`; sla-lowering
//! has not ported this family, so — following the D3 precedent
//! in `copy_facts.rs` (base facts ported locally, marked for
//! re-homing) — the plans used by the whole E group are ported
//! here line-for-line and marked for re-homing into
//! sla-lowering when that lane picks them up.
//!
//! Param cleanup facts (`paramCleanupAction`) are computed
//! here too: E5's release walkers consume them, and the facts
//! themselves are release metadata.

use sla_ast::Type;
use sla_lowering::CheckInfo;

use crate::captures::is_local_reg;
use crate::copy_facts::CopyFacts;
use crate::error::Result;
use crate::ir::{CapPrefix, PrimType};
use crate::layout::prim_type;
use crate::release_emit::{
    emit_move, emit_release, mark_consumed, release_non_local_temps, MacroFragmentEmitter,
};
use crate::release_prim::{emit_alloc, emit_load, emit_store};
use crate::state::{CodegenState, RegState};
use crate::types::{AddressSource, Local, RefCellBorrowKind, ResultSlotRefCellHandle};

use crate::release_plans::{
    plan_borrow_address_temps, plan_param_cleanup, plan_refcell_companion_restore,
    plan_refcell_companion_store_cleanup, plan_refcell_handle_owner_transfer,
    plan_refcell_value_state_transfer, plan_result_slot_load_lifecycle,
    plan_result_slot_refcell_load, plan_result_slot_refcell_store,
    plan_result_slot_store_lifecycle, plan_result_slot_transfer, ParamCleanupAction,
    ParamCleanupCapability, ParamCleanupFacts,
};
// ----------------------------------------------------------------
// Codegen-side metadata operations (Zig Codegen methods)
// ----------------------------------------------------------------

/// Zig `paramCleanupAction`: build the facts for a local and
/// plan its parameter cleanup. `primType` failure propagates
/// (Zig `try primType`) — the caller decides loudness.
pub fn param_cleanup_action(
    check: &dyn CheckInfo,
    local: &Local,
) -> Result<ParamCleanupAction> {
    let mut facts_out = CopyFacts::new(check);
    let facts = ParamCleanupFacts {
        is_param: local.is_param,
        capability: match local.param_cap {
            Some(CapPrefix::Raw) => ParamCleanupCapability::Raw,
            Some(CapPrefix::Borrow) => ParamCleanupCapability::Borrow,
            Some(CapPrefix::ByValue) => ParamCleanupCapability::ByValue,
            Some(CapPrefix::Move) => ParamCleanupCapability::Other,
            None => ParamCleanupCapability::None,
        },
        has_type: local.ty.is_some(),
        abi_is_ptr: match &local.ty {
            Some(ty) => prim_type(ty)? == PrimType::Ptr,
            None => false,
        },
        is_copy_value: match &local.ty {
            Some(ty) => facts_out.type_is_copy_value(ty),
            None => false,
        },
        is_fn_ptr: matches!(&local.ty, Some(Type::FnPtr(_))),
    };
    Ok(plan_param_cleanup(facts))
}

/// Zig `singleReleaseReg`.
pub fn single_release_reg(reg: u32) -> Vec<u32> {
    vec![reg]
}

/// Zig `ownedReleaseRegs`.
pub fn owned_release_regs(regs: &[u32]) -> Vec<u32> {
    regs.to_vec()
}

/// Zig `markNonOwningReg`: the register becomes a non-owning
/// view (SAB-D09 classification write).
pub fn mark_non_owning_reg(state: &mut CodegenState, reg: u32) {
    state.frame.reg_states.insert(reg, RegState::NonOwning);
}

/// Zig `rememberBorrowAddressTemps`: remember the source
/// temps of a borrow-address register per the plan; the
/// restore value is the source register itself whenever a
/// restore slot exists.
pub fn remember_borrow_address_temps(
    state: &mut CodegenState,
    borrow_reg: u32,
    source: &AddressSource,
) {
    let plan = plan_borrow_address_temps(!is_local_reg(state, source.reg), !source.release_regs.is_empty());
    if !plan.remember && source.restore_slot.is_none() {
        return;
    }
    let mut regs: Vec<u32> = Vec::new();
    if plan.track_primary_temp {
        regs.push(source.reg);
    }
    if plan.track_extra_temps {
        regs.extend_from_slice(&source.release_regs);
    }
    state.frame.borrow_address_temps.insert(
        borrow_reg,
        crate::types::BorrowAddressTempState {
            release_regs: regs,
            restore_slot: source.restore_slot,
            restore_value: if source.restore_slot.is_some() {
                Some(source.reg)
            } else {
                None
            },
        },
    );
}

/// Zig `rebindRefCellBorrowValueOwners`: every RefCell handle
/// owned by `src` is rebound to `dst`.
pub fn rebind_refcell_borrow_value_owners(state: &mut CodegenState, src: u32, dst: u32) {
    if src == dst {
        return;
    }
    for handle in state.frame.refcell_borrow_values.values_mut() {
        let transfer = plan_refcell_handle_owner_transfer(handle.cell_reg == src);
        if transfer.rebinds_owner() {
            handle.cell_reg = dst;
        }
    }
}

/// Zig `transferReleaseMetadata`: move all release metadata
/// (RefCell handle + borrow temps, guards, non-owning mark)
/// from `src` to `dst`.
pub fn transfer_release_metadata(state: &mut CodegenState, dst: u32, src: u32) -> Result<()> {
    if dst == src {
        return Ok(());
    }
    rebind_refcell_borrow_value_owners(state, src, dst);
    let plan = plan_refcell_value_state_transfer(
        state.frame.refcell_borrow_values.contains_key(&src),
        state.frame.borrow_address_temps.contains_key(&src),
    );
    if plan.handle.moves_borrow_handle() {
        if let Some(entry) = state.frame.refcell_borrow_values.remove(&src) {
            state.frame.refcell_borrow_values.remove(&dst);
            state.frame.refcell_borrow_values.insert(dst, entry);
        }
    }
    if let Some(entry) = state.frame.mutex_guard_values.remove(&src) {
        state.frame.mutex_guard_values.remove(&dst);
        state.frame.mutex_guard_values.insert(dst, entry);
    }
    if let Some(entry) = state.frame.rwlock_guard_values.remove(&src) {
        state.frame.rwlock_guard_values.remove(&dst);
        state.frame.rwlock_guard_values.insert(dst, entry);
    }
    if plan.borrow_address_temps.moves_borrow_address_temps() {
        if let Some(entry) = state.frame.borrow_address_temps.remove(&src) {
            state.frame.borrow_address_temps.remove(&dst);
            state.frame.borrow_address_temps.insert(dst, entry);
        }
    }
    if state.frame.reg_state(src) == RegState::NonOwning {
        state.frame.reg_states.remove(&src);
        mark_non_owning_reg(state, dst);
    }
    Ok(())
}

/// Zig `transferResultSlotValueState`: release metadata plus
/// future vtable/readiness, optionally consuming the source.
pub fn transfer_result_slot_value_state(
    state: &mut CodegenState,
    dst: u32,
    src: u32,
    consume_src: bool,
) -> Result<()> {
    if dst == src {
        return Ok(());
    }
    transfer_release_metadata(state, dst, src)?;
    crate::release_emit::transfer_future_state_vtable(state, src, dst);
    crate::release_emit::transfer_future_readiness(state, src, dst);
    if consume_src {
        mark_consumed(state, src);
    }
    Ok(())
}

/// Zig `ensureResultSlotRefCellSlot`: the result slot's
/// RefCell companion cell (an 8-byte alloc), created once.
pub fn ensure_result_slot_refcell_slot(state: &mut CodegenState, slot: u32) -> Result<u32> {
    if let Some(existing) = state.frame.result_slot_refcell_slots.get(&slot) {
        return Ok(*existing);
    }
    let cell_slot = state.builder.new_tmp();
    emit_alloc(state, cell_slot, 8);
    state.frame.result_slot_refcell_slots.insert(slot, cell_slot);
    Ok(cell_slot)
}

/// Zig `prepareResultSlotRefCellCompanion`.
pub fn prepare_result_slot_refcell_companion(
    state: &mut CodegenState,
    slot: u32,
    target_ty: &Type,
) -> Result<()> {
    if !plan_result_slot_transfer(target_ty).needs_refcell_companion {
        return Ok(());
    }
    ensure_result_slot_refcell_slot(state, slot)?;
    Ok(())
}

/// Zig `ensureResultSlotRefCellHandle`: the slot's handle
/// metadata; an existing handle's kind is updated in place.
pub fn ensure_result_slot_refcell_handle(
    state: &mut CodegenState,
    slot: u32,
    kind: RefCellBorrowKind,
) -> Result<ResultSlotRefCellHandle> {
    if let Some(existing) = state.frame.result_slot_refcell_handles.get(&slot).copied() {
        let updated = ResultSlotRefCellHandle {
            cell_slot: existing.cell_slot,
            kind,
        };
        state.frame.result_slot_refcell_handles.insert(slot, updated);
        return Ok(updated);
    }
    let cell_slot = ensure_result_slot_refcell_slot(state, slot)?;
    let meta = ResultSlotRefCellHandle { cell_slot, kind };
    state.frame.result_slot_refcell_handles.insert(slot, meta);
    Ok(meta)
}

/// Zig `storeResultSlotTransferredValue`.
pub fn store_result_slot_transferred_value(
    state: &mut CodegenState,
    frags: &mut dyn MacroFragmentEmitter,
    slot: u32,
    src: u32,
    target_ty: &Type,
) -> Result<()> {
    let plan = plan_result_slot_transfer(target_ty);
    let lifecycle = plan_result_slot_store_lifecycle(plan, !is_local_reg(state, src));
    if lifecycle.releases_source() {
        return emit_release(state, frags, src);
    }
    if !lifecycle.transfers_value_state() {
        return Ok(());
    }
    let refcell_store =
        plan_result_slot_refcell_store(plan, state.frame.refcell_borrow_values.contains_key(&src));
    if refcell_store.stores_borrow_handle_companion() {
        if let Some(entry) = state.frame.refcell_borrow_values.remove(&src) {
            let meta = ensure_result_slot_refcell_handle(state, slot, entry.kind)?;
            emit_store(state, meta.cell_slot, 0, entry.cell_reg, PrimType::Ptr);
            let cleanup = plan_refcell_companion_store_cleanup(
                !entry.release_regs.is_empty(),
                state.frame.borrow_address_temps.contains_key(&src),
                state.frame.reg_state(src) == RegState::NonOwning,
            );
            if cleanup.release_owner_temps {
                release_non_local_temps(state, frags, &entry.release_regs)?;
            }
            if cleanup.release_borrow_address_temps {
                if let Some(temps) = state.frame.borrow_address_temps.remove(&src) {
                    if let Some(restore_slot) = temps.restore_slot {
                        let restore_value = temps.restore_value.ok_or_else(|| {
                            crate::error::Error::UnsupportedSabDirectFeature(
                                "borrow address temp has restore slot but no restore value [group E4 release_meta]"
                                    .to_string(),
                            )
                        })?;
                        emit_store(state, restore_slot, 0, restore_value, PrimType::Ptr);
                        mark_consumed(state, restore_value);
                    }
                    release_non_local_temps(state, frags, &temps.release_regs)?;
                }
            }
            if cleanup.clear_non_owning_metadata
                && state.frame.reg_state(src) == RegState::NonOwning
            {
                state.frame.reg_states.remove(&src);
            }
            if cleanup.consume_handle_value {
                mark_consumed(state, src);
            }
            return Ok(());
        }
    }
    emit_move_for_slot_transfer(state, frags, src)?;
    transfer_result_slot_value_state(state, slot, src, true)
}

/// Zig `emitMoveForSlotTransfer`: locals are exempt from the
/// slot-transfer `move_`.
pub fn emit_move_for_slot_transfer(
    state: &mut CodegenState,
    _frags: &mut dyn MacroFragmentEmitter,
    src: u32,
) -> Result<()> {
    if !is_local_reg(state, src) {
        emit_move(state, src);
    }
    Ok(())
}

/// Zig `loadResultSlotTransferredValue`.
pub fn load_result_slot_transferred_value(
    state: &mut CodegenState,
    frags: &mut dyn MacroFragmentEmitter,
    dst: u32,
    slot: u32,
    target_ty: &Type,
) -> Result<()> {
    let plan = plan_result_slot_transfer(target_ty);
    if !plan_result_slot_load_lifecycle(plan).loads_value_state() {
        return Ok(());
    }
    let refcell_load = plan_result_slot_refcell_load(
        plan,
        state.frame.result_slot_refcell_handles.contains_key(&slot),
        state.frame.result_slot_refcell_slots.contains_key(&slot),
    );
    if refcell_load.restores_borrow_handle_companion() {
        if let Some(entry) = state.frame.result_slot_refcell_handles.remove(&slot) {
            state.frame.result_slot_refcell_slots.remove(&slot);
            let cell_reg = state.builder.new_tmp();
            let restore = plan_refcell_companion_restore();
            emit_load(state, cell_reg, entry.cell_slot, 0, PrimType::Ptr);
            let release_regs = if restore.track_loaded_cell_owner_temp {
                single_release_reg(cell_reg)
            } else {
                Vec::new()
            };
            state.frame.refcell_borrow_values.insert(
                dst,
                crate::types::RefCellBorrowValue {
                    cell_reg,
                    kind: entry.kind,
                    release_regs,
                },
            );
            if restore.release_companion_slot_after_restore {
                emit_release(state, frags, entry.cell_slot)?;
            }
        }
    } else if refcell_load.releases_empty_companion() {
        if let Some(cell_slot) = state.frame.result_slot_refcell_slots.remove(&slot) {
            emit_release(state, frags, cell_slot)?;
        }
    }
    transfer_result_slot_value_state(state, dst, slot, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release_plans::{
        plan_branch_state_merge, plan_multi_branch_state_merge, BranchStateMergeAction,
        MultiBranchStateMergeAction,
    };
    use sla_ast::{Primitive, UserDefined};
    use sla_lowering::MapCheckInfo;

    fn facts(cap: ParamCleanupCapability) -> ParamCleanupFacts {
        ParamCleanupFacts {
            is_param: true,
            capability: cap,
            has_type: true,
            abi_is_ptr: false,
            is_copy_value: false,
            is_fn_ptr: false,
        }
    }

    #[test]
    fn param_cleanup_plan_table() {
        assert_eq!(plan_param_cleanup(facts(ParamCleanupCapability::Raw)), ParamCleanupAction::Skip);
        assert_eq!(plan_param_cleanup(facts(ParamCleanupCapability::Borrow)), ParamCleanupAction::Release);
        let mut f = facts(ParamCleanupCapability::ByValue);
        f.abi_is_ptr = true;
        assert_eq!(plan_param_cleanup(f), ParamCleanupAction::Consume);
        f.abi_is_ptr = false;
        f.is_copy_value = true;
        assert_eq!(plan_param_cleanup(f), ParamCleanupAction::Release);
        let mut non_param = facts(ParamCleanupCapability::Raw);
        non_param.is_param = false;
        assert_eq!(plan_param_cleanup(non_param), ParamCleanupAction::Release);
    }

    #[test]
    fn branch_merge_plans() {
        assert_eq!(plan_branch_state_merge(true, true), BranchStateMergeAction::RestorePre);
        assert_eq!(plan_branch_state_merge(true, false), BranchStateMergeAction::RestoreElse);
        assert_eq!(plan_branch_state_merge(false, true), BranchStateMergeAction::RestoreThen);
        assert_eq!(plan_branch_state_merge(false, false), BranchStateMergeAction::KeepCurrent);
        assert_eq!(plan_multi_branch_state_merge(0), MultiBranchStateMergeAction::RestorePre);
        assert_eq!(plan_multi_branch_state_merge(1), MultiBranchStateMergeAction::RestoreSingle);
        assert_eq!(plan_multi_branch_state_merge(3), MultiBranchStateMergeAction::IntersectLive);
    }

    #[test]
    fn result_slot_transfer_plan_ptr_vs_scalar() {
        let ptr_ty = Type::UserDefined(UserDefined {
            name: "Vec".to_string(),
            generics: vec![Type::Primitive(Primitive::I32)],
        });
        assert!(plan_result_slot_transfer(&ptr_ty).transfers_value);
        let scalar = Type::Primitive(Primitive::I32);
        assert!(!plan_result_slot_transfer(&scalar).transfers_value);
        let borrow = Type::Borrow(Box::new(scalar.clone()));
        assert!(plan_result_slot_transfer(&borrow).needs_refcell_companion);
    }

    #[test]
    fn param_cleanup_action_for_borrow_param_releases() {
        let check = MapCheckInfo::new();
        let mut local = Local::typed("p", 1, true, Type::Primitive(Primitive::I32));
        local.param_cap = Some(CapPrefix::Borrow);
        assert_eq!(param_cleanup_action(&check, &local).unwrap(), ParamCleanupAction::Release);
    }

    #[test]
    fn transfer_metadata_moves_handle_and_nonowning() {
        let mut st = CodegenState::new();
        st.frame.refcell_borrow_values.insert(
            10,
            crate::types::RefCellBorrowValue {
                cell_reg: 99,
                kind: RefCellBorrowKind::Shared,
                release_regs: vec![],
            },
        );
        mark_non_owning_reg(&mut st, 10);
        transfer_release_metadata(&mut st, 11, 10).unwrap();
        assert!(st.frame.refcell_borrow_values.contains_key(&11));
        assert!(!st.frame.refcell_borrow_values.contains_key(&10));
        assert_eq!(st.frame.reg_state(11), RegState::NonOwning);
    }

    #[test]
    fn ensure_slot_allocates_once() {
        let mut st = CodegenState::new();
        let a = ensure_result_slot_refcell_slot(&mut st, 5).unwrap();
        let b = ensure_result_slot_refcell_slot(&mut st, 5).unwrap();
        assert_eq!(a, b);
        assert_eq!(st.builder.instructions_len(), 1);
    }
}
