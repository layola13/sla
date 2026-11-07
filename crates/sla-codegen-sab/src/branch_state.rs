//! E3 — branch emitter-state snapshots, mirroring
//! `sab_codegen.zig` lines 2602–2952 in the reference tree
//! (`/content/sa_all`, HEAD 2cc12ea8): the borrow-temp and
//! RefCell-value clone/restore family, `cloneBranchEmitterState`,
//! the multi-branch merge (`setMergeBranchEmitterState`), the
//! two-branch RefCell merge (`setMergeRefCellBranchState`), the
//! RefCell branch-handle owner merge slots, and the released-set
//! restore/merge pair (`restoreReleased`/`setMergeReleased`,
//! lines 3964–4030). (Decomposition doc lines 2430–2767 in the
//! older tree.)
//!
//! Form notes:
//! - Zig snapshots own heap slices and are freed by explicit
//!   `deinit*Snapshot` calls; here snapshots are plain owned
//!   values (RAII) — the clone/restore *contents* are identical.
//! - The Zig `released_regs` set is the frame's SAB-D09
//!   classification here: a snapshot's `released` set is
//!   materialized from `RegState::Released` entries, and a
//!   restore rewrites exactly those entries.
//! - Merge-slot emission walks the slot map in ascending
//!   register order; Zig walks hash order (non-deterministic).
//!   The emitted stores/loads commute (disjoint slots), and the
//!   port never reproduces hash order — registered as
//!   SAB-D101 in the E-group REPORT (same entry as E7's
//!   RefCell-cell release walk).

use std::collections::{HashMap, HashSet};

use crate::error::Result;
use crate::ir::PrimType;
use crate::release_emit::{emit_release, MacroFragmentEmitter};
use crate::release_meta::single_release_reg;
use crate::release_plans::{
    plan_multi_branch_state_merge, plan_refcell_branch_handle_owner_merge,
    plan_refcell_branch_state_merge, plan_refcell_companion_restore,
};
use crate::release_prim::{emit_alloc, emit_load, emit_store};
use crate::state::{CodegenState, RegState};
use crate::types::{
    BorrowAddressTempState, BranchEmitterStateSnapshot, BranchRefCellHandleOwnerMergeSlot,
    RefCellBorrowValue,
};

/// Zig `cloneBorrowAddressTemps`: a deep copy (release-reg
/// lists are duplicated, as Zig's `allocator.dupe` does).
pub fn clone_borrow_address_temps(
    state: &CodegenState,
) -> HashMap<u32, BorrowAddressTempState> {
    state.frame.borrow_address_temps.clone()
}

/// Zig `restoreBorrowAddressTemps`: clear, then deep-copy the
/// snapshot in. (Zig's clear also empties `cell_target_borrows`,
/// a current-head set this port's frame does not model — see
/// the E-group REPORT's state notes.)
pub fn restore_borrow_address_temps(
    state: &mut CodegenState,
    snapshot: &HashMap<u32, BorrowAddressTempState>,
) {
    state.frame.borrow_address_temps.clear();
    state.frame.borrow_address_temps.clone_from(snapshot);
}

/// Zig `cloneRefCellBorrowValues`: a deep copy.
pub fn clone_refcell_borrow_values(
    state: &CodegenState,
) -> HashMap<u32, RefCellBorrowValue> {
    state.frame.refcell_borrow_values.clone()
}

/// Zig `restoreRefCellBorrowValues`: clear, then deep-copy in.
pub fn restore_refcell_borrow_values(
    state: &mut CodegenState,
    snapshot: &HashMap<u32, RefCellBorrowValue>,
) {
    state.frame.refcell_borrow_values.clear();
    state.frame.refcell_borrow_values.clone_from(snapshot);
}

/// The frame's current released set, materialized (Zig
/// `released_regs.clone()` inside `cloneBranchEmitterState`).
pub fn clone_released(state: &CodegenState) -> HashSet<u32> {
    state
        .frame
        .reg_states
        .iter()
        .filter(|(_, s)| **s == RegState::Released)
        .map(|(reg, _)| *reg)
        .collect()
}

/// Zig `restoreReleased`: rebuild the released set to match
/// the snapshot exactly.
pub fn restore_released(state: &mut CodegenState, snapshot: &HashSet<u32>) {
    state
        .frame
        .reg_states
        .retain(|_, s| *s != RegState::Released);
    for reg in snapshot {
        state.frame.reg_states.insert(*reg, RegState::Released);
    }
}

/// Zig `setMergeReleased`: the merge point's released set is
/// the intersection of the fall-through branches' sets; when
/// one branch terminated, the other's set wins; when both
/// terminated the merge is dead and the pre-state is kept.
pub fn set_merge_released(
    state: &mut CodegenState,
    then_terminated: bool,
    then_released: &HashSet<u32>,
    else_terminated: bool,
    else_released: &HashSet<u32>,
    pre_released: &HashSet<u32>,
) {
    if then_terminated && else_terminated {
        restore_released(state, pre_released);
        return;
    }
    if then_terminated {
        restore_released(state, else_released);
        return;
    }
    if else_terminated {
        restore_released(state, then_released);
        return;
    }
    let intersection: HashSet<u32> = then_released.intersection(else_released).copied().collect();
    restore_released(state, &intersection);
}

/// Zig `cloneBranchEmitterState`.
pub fn clone_branch_emitter_state(state: &CodegenState) -> BranchEmitterStateSnapshot {
    BranchEmitterStateSnapshot {
        released: clone_released(state),
        refcell_values: clone_refcell_borrow_values(state),
        borrow_temps: clone_borrow_address_temps(state),
    }
}

/// Zig `appendCurrentBranchEmitterState`.
pub fn append_current_branch_emitter_state(
    state: &CodegenState,
    snapshots: &mut Vec<BranchEmitterStateSnapshot>,
) {
    snapshots.push(clone_branch_emitter_state(state));
}

/// Zig `refCellBorrowValueEqual`.
pub fn refcell_borrow_value_equal(left: &RefCellBorrowValue, right: &RefCellBorrowValue) -> bool {
    left.cell_reg == right.cell_reg
        && left.kind == right.kind
        && left.release_regs == right.release_regs
}

fn borrow_temp_state_equal(left: &BorrowAddressTempState, right: &BorrowAddressTempState) -> bool {
    left.release_regs == right.release_regs
        && left.restore_slot == right.restore_slot
        && left.restore_value == right.restore_value
}

/// Zig `restoreRefCellBranchState`: restore both payload maps
/// from a branch snapshot pair.
pub fn restore_refcell_branch_state(
    state: &mut CodegenState,
    values: &HashMap<u32, RefCellBorrowValue>,
    temps: &HashMap<u32, BorrowAddressTempState>,
) {
    restore_refcell_borrow_values(state, values);
    restore_borrow_address_temps(state, temps);
}

/// Zig `setMergeBranchEmitterState`: 0 live branches restore
/// the pre-state, 1 restores that branch, ≥2 intersect —
/// released regs, RefCell values and borrow temps are kept
/// only where every live branch agrees.
pub fn set_merge_branch_emitter_state(
    state: &mut CodegenState,
    live_snapshots: &[BranchEmitterStateSnapshot],
    pre_snapshot: &BranchEmitterStateSnapshot,
) {
    let action = plan_multi_branch_state_merge(live_snapshots.len());
    if action.restores_pre() {
        restore_released(state, &pre_snapshot.released);
        restore_refcell_branch_state(state, &pre_snapshot.refcell_values, &pre_snapshot.borrow_temps);
    } else if action.restores_single() {
        let only = &live_snapshots[0];
        restore_released(state, &only.released);
        restore_refcell_branch_state(state, &only.refcell_values, &only.borrow_temps);
    } else if action.intersects_live() {
        let first = &live_snapshots[0];
        let rest = &live_snapshots[1..];
        let released: HashSet<u32> = first
            .released
            .iter()
            .copied()
            .filter(|reg| rest.iter().all(|s| s.released.contains(reg)))
            .collect();
        restore_released(state, &released);

        let mut values: HashMap<u32, RefCellBorrowValue> = HashMap::new();
        for (reg, value) in &first.refcell_values {
            let shared = rest.iter().all(|s| {
                s.refcell_values
                    .get(reg)
                    .map(|other| refcell_borrow_value_equal(value, other))
                    .unwrap_or(false)
            });
            if shared {
                values.insert(*reg, value.clone());
            }
        }
        state.frame.refcell_borrow_values = values;

        let mut temps: HashMap<u32, BorrowAddressTempState> = HashMap::new();
        for (reg, temp) in &first.borrow_temps {
            let shared = rest.iter().all(|s| {
                s.borrow_temps
                    .get(reg)
                    .map(|other| borrow_temp_state_equal(temp, other))
                    .unwrap_or(false)
            });
            if shared {
                temps.insert(*reg, temp.clone());
            }
        }
        state.frame.borrow_address_temps = temps;
    }
}

/// Zig `setMergeRefCellBranchState`: the two-branch RefCell
/// merge — both-terminated restores pre, a terminated branch
/// cedes to the live one, both-live keeps the current state
/// (`keep_current` has no restore, exactly as in Zig).
pub fn set_merge_refcell_branch_state(
    state: &mut CodegenState,
    then_terminated: bool,
    then_values: &HashMap<u32, RefCellBorrowValue>,
    then_temps: &HashMap<u32, BorrowAddressTempState>,
    else_terminated: bool,
    else_values: &HashMap<u32, RefCellBorrowValue>,
    else_temps: &HashMap<u32, BorrowAddressTempState>,
    pre_values: &HashMap<u32, RefCellBorrowValue>,
    pre_temps: &HashMap<u32, BorrowAddressTempState>,
) {
    let action = plan_refcell_branch_state_merge(then_terminated, else_terminated);
    if action.restores_pre() {
        restore_refcell_branch_state(state, pre_values, pre_temps);
    } else if action.restores_then() {
        restore_refcell_branch_state(state, then_values, then_temps);
    } else if action.restores_else() {
        restore_refcell_branch_state(state, else_values, else_temps);
    }
}

/// Zig `prepareRefCellBranchHandleOwnerMergeSlots`: allocate
/// an 8-byte owner-merge slot per pre-branch RefCell handle.
/// (Zig fills a caller-provided map; the port returns it.)
pub fn prepare_refcell_branch_handle_owner_merge_slots(
    state: &mut CodegenState,
    pre_values: &HashMap<u32, RefCellBorrowValue>,
) -> HashMap<u32, BranchRefCellHandleOwnerMergeSlot> {
    let mut slots = HashMap::new();
    let mut keys: Vec<u32> = pre_values.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let cell_slot = state.builder.new_tmp();
        emit_alloc(state, cell_slot, 8);
        slots.insert(
            key,
            BranchRefCellHandleOwnerMergeSlot {
                cell_slot,
                kind: pre_values[&key].kind,
            },
        );
    }
    slots
}

/// Zig `storeRefCellBranchHandleOwnerMergeSlots`: store each
/// live handle's cell register into its merge slot.
pub fn store_refcell_branch_handle_owner_merge_slots(
    state: &mut CodegenState,
    slots: &HashMap<u32, BranchRefCellHandleOwnerMergeSlot>,
    values: &HashMap<u32, RefCellBorrowValue>,
) {
    let mut keys: Vec<u32> = slots.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        if let Some(value) = values.get(&key) {
            emit_store(state, slots[&key].cell_slot, 0, value.cell_reg, PrimType::Ptr);
        }
    }
}

/// Zig `loadRefCellBranchHandleOwnerMergeSlots`: after the
/// merge, a handle live on both sides with the same kind but
/// different owners is reloaded from its merge slot as a
/// dynamic-owner handle; every other slot is simply released.
pub fn load_refcell_branch_handle_owner_merge_slots(
    state: &mut CodegenState,
    frags: &mut dyn MacroFragmentEmitter,
    slots: &HashMap<u32, BranchRefCellHandleOwnerMergeSlot>,
    then_terminated: bool,
    then_values: &HashMap<u32, RefCellBorrowValue>,
    else_terminated: bool,
    else_values: &HashMap<u32, RefCellBorrowValue>,
) -> Result<()> {
    let mut keys: Vec<u32> = slots.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let slot = slots[&key];
        let then_value = if !then_terminated { then_values.get(&key) } else { None };
        let else_value = if !else_terminated { else_values.get(&key) } else { None };
        let action = plan_refcell_branch_handle_owner_merge(
            then_value.is_some(),
            else_value.is_some(),
            matches!((then_value, else_value), (Some(t), Some(e)) if t.kind == e.kind),
            matches!((then_value, else_value), (Some(t), Some(e)) if t.cell_reg == e.cell_reg),
        );
        if action.is_merge_dynamic_owner() {
            let cell_reg = state.builder.new_tmp();
            let restore_plan = plan_refcell_companion_restore();
            emit_load(state, cell_reg, slot.cell_slot, 0, PrimType::Ptr);
            state.frame.refcell_borrow_values.remove(&key);
            let release_regs = if restore_plan.track_loaded_cell_owner_temp {
                single_release_reg(cell_reg)
            } else {
                Vec::new()
            };
            state.frame.refcell_borrow_values.insert(
                key,
                RefCellBorrowValue {
                    cell_reg,
                    kind: then_value.expect("dynamic owner merge implies a then value").kind,
                    release_regs,
                },
            );
            if restore_plan.release_companion_slot_after_restore {
                emit_release(state, frags, slot.cell_slot)?;
            }
        } else {
            emit_release(state, frags, slot.cell_slot)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::InstKind;
    use crate::release_emit::NoMacroFragments;
    use crate::types::RefCellBorrowKind;

    fn handle(cell: u32) -> RefCellBorrowValue {
        RefCellBorrowValue {
            cell_reg: cell,
            kind: RefCellBorrowKind::Shared,
            release_regs: vec![cell + 100],
        }
    }

    #[test]
    fn snapshot_restore_roundtrip() {
        let mut st = CodegenState::new();
        st.frame.refcell_borrow_values.insert(1, handle(50));
        crate::release_prim::mark_released(&mut st, 7);
        let snap = clone_branch_emitter_state(&st);
        st.frame.refcell_borrow_values.clear();
        crate::branch_state::restore_released(&mut st, &HashSet::new());
        restore_refcell_branch_state(&mut st, &snap.refcell_values, &snap.borrow_temps);
        assert_eq!(st.frame.refcell_borrow_values[&1].cell_reg, 50);
        assert!(snap.released.contains(&7));
    }

    #[test]
    fn merge_released_intersects_live_branches() {
        let mut st = CodegenState::new();
        let then_r: HashSet<u32> = [1, 2, 3].into_iter().collect();
        let else_r: HashSet<u32> = [2, 3, 4].into_iter().collect();
        let pre: HashSet<u32> = [9].into_iter().collect();
        set_merge_released(&mut st, false, &then_r, false, &else_r, &pre);
        assert_eq!(clone_released(&st), [2, 3].into_iter().collect());
        set_merge_released(&mut st, true, &then_r, false, &else_r, &pre);
        assert_eq!(clone_released(&st), else_r);
        set_merge_released(&mut st, true, &then_r, true, &else_r, &pre);
        assert_eq!(clone_released(&st), pre);
    }

    #[test]
    fn multi_branch_merge_intersects_values() {
        let mut st = CodegenState::new();
        let mut a = BranchEmitterStateSnapshot::default();
        a.refcell_values.insert(1, handle(50));
        a.refcell_values.insert(2, handle(60));
        let mut b = BranchEmitterStateSnapshot::default();
        b.refcell_values.insert(1, handle(50));
        b.refcell_values.insert(2, handle(61));
        let pre = BranchEmitterStateSnapshot::default();
        set_merge_branch_emitter_state(&mut st, &[a, b], &pre);
        assert!(st.frame.refcell_borrow_values.contains_key(&1));
        assert!(!st.frame.refcell_borrow_values.contains_key(&2));
    }

    #[test]
    fn merge_slots_prepare_store_load_static_owner() {
        let mut st = CodegenState::new();
        let mut pre = HashMap::new();
        pre.insert(1, handle(50));
        let slots = prepare_refcell_branch_handle_owner_merge_slots(&mut st, &pre);
        assert_eq!(slots.len(), 1);
        store_refcell_branch_handle_owner_merge_slots(&mut st, &slots, &pre);
        let mut frags = NoMacroFragments;
        // Same owner on both sides -> keep static owner: the
        // slot is branch-released, no handle is installed.
        load_refcell_branch_handle_owner_merge_slots(&mut st, &mut frags, &slots, false, &pre, false, &pre)
            .unwrap();
        assert!(st.frame.refcell_borrow_values.is_empty());
        let kinds: Vec<InstKind> = st
            .builder
            .finish()
            .instructions
            .iter()
            .map(|i| i.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![InstKind::Alloc, InstKind::Store, InstKind::Release]
        );
    }
}
