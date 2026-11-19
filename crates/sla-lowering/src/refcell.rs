//! RefCell borrow planning, mirrored from lowering_rules.zig's
//! `planRefCellBorrowCall` family (5484), the borrow-result and
//! runtime-guard plans (3841-3892), and the handle release plans
//! (3801-3813). These are the pure decision tables the SA text
//! emitter needs to materialize `cell.borrow()` /
//! `cell.borrow_mut()`: the try-borrow expansion with its panic
//! guard, the payload shape of the borrow result, and the
//! release of a live borrow handle at its cleanup.
//!
//! The remaining RefCell state-machine plans (value-state
//! transfer, loop-state merge) stay registered in deferred.rs.

use sla_ast::{Primitive, Type};

use crate::abi::peel_borrow_pointer_type;

/// lowering_rules.zig `RefCellBorrowKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefCellBorrowKind {
    Shared,
    Mutable,
}

impl RefCellBorrowKind {
    /// lowering_rules.zig `tryBorrowMacroName` (3973 area): the
    /// expansion that opens the borrow.
    pub fn try_borrow_macro_name(&self) -> &'static str {
        match self {
            RefCellBorrowKind::Mutable => "REFCELL_U64_TRY_BORROW_MUT",
            RefCellBorrowKind::Shared => "REFCELL_U64_TRY_BORROW",
        }
    }

    /// lowering_rules.zig:3973 `refCellBorrowReleaseMacroName`:
    /// the expansion that settles the borrow at cleanup.
    pub fn release_macro_name(&self) -> &'static str {
        match self {
            RefCellBorrowKind::Shared => "REFCELL_U64_RELEASE_SHARED",
            RefCellBorrowKind::Mutable => "REFCELL_U64_RELEASE_MUT",
        }
    }
}

/// lowering_rules.zig `RefCellBorrowValueKind`: what the cell's
/// payload word holds for the borrow result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefCellBorrowValueKind {
    /// The payload is a scalar stored in the borrow slot itself.
    ScalarSlot,
    /// The payload word is a pointer to the value.
    PointerPayload,
    /// The payload word is a Box/Rc/Arc handle to the value.
    SmartPointerPayload,
}

/// lowering_rules.zig:5049 `refCellInnerType`.
pub fn refcell_inner_type(ty: &Type) -> Option<&Type> {
    let curr = peel_borrow_pointer_type(ty);
    let Type::UserDefined(u) = curr else {
        return None;
    };
    if u.name != "RefCell" || u.generics.len() != 1 {
        return None;
    }
    Some(&u.generics[0])
}

/// Whether the type is (or peels to) a Box/Rc/Arc handle — the
/// deref-capable half of lowering_rules.zig:5166
/// `smartPointerDerefType` (its RefCell case yields null there).
fn smart_pointer_deref_name(ty: &Type) -> bool {
    let curr = peel_borrow_pointer_type(ty);
    matches!(curr, Type::UserDefined(u) if matches!(u.name.as_str(), "Box" | "Rc" | "Arc"))
}

/// lowering_rules.zig:5471 `refCellPayloadIsPointer`: only a
/// non-raw-pointer primitive payload lives in the slot itself;
/// every other payload shape is carried by pointer.
pub fn refcell_payload_is_pointer(ty: &Type) -> bool {
    match ty {
        Type::Primitive(p) => *p == Primitive::RawPtr,
        _ => true,
    }
}

/// lowering_rules.zig:5478 `planRefCellBorrowValueKind`.
pub fn plan_refcell_borrow_value_kind(inner: &Type) -> RefCellBorrowValueKind {
    if smart_pointer_deref_name(inner) {
        return RefCellBorrowValueKind::SmartPointerPayload;
    }
    if refcell_payload_is_pointer(inner) {
        return RefCellBorrowValueKind::PointerPayload;
    }
    RefCellBorrowValueKind::ScalarSlot
}

/// lowering_rules.zig `RefCellBorrowPlan` (the SA-text fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefCellBorrowPlan {
    pub kind: RefCellBorrowKind,
    pub value_kind: RefCellBorrowValueKind,
}

/// lowering_rules.zig:5484 `planRefCellBorrowCall`: a one
/// argument `borrow` / `borrow_mut` call on a RefCell receiver.
pub fn plan_refcell_borrow_call(
    func_name: &str,
    receiver_ty: &Type,
    arg_count: usize,
) -> Option<RefCellBorrowPlan> {
    if arg_count != 1 {
        return None;
    }
    let inner = refcell_inner_type(receiver_ty)?;
    let value_kind = plan_refcell_borrow_value_kind(inner);
    let kind = match func_name {
        "borrow" => RefCellBorrowKind::Shared,
        "borrow_mut" => RefCellBorrowKind::Mutable,
        _ => return None,
    };
    Some(RefCellBorrowPlan { kind, value_kind })
}

/// lowering_rules.zig `RefCellBorrowResultPlan` action for the
/// SA text target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefCellBorrowResultAction {
    /// The borrow result IS the borrow slot register.
    UseBorrowSlot,
    /// The borrow result is the pointer stored in the slot.
    LoadPointerPayload,
}

/// lowering_rules.zig:3841 `planRefCellBorrowResult` (sa_text
/// arm): scalar payloads use the slot directly; pointer and
/// smart-pointer payloads load the pointer out of the slot and
/// the slot register is released right after the load.
pub fn plan_refcell_borrow_result_sa_text(
    value_kind: RefCellBorrowValueKind,
) -> (RefCellBorrowResultAction, bool) {
    match value_kind {
        RefCellBorrowValueKind::ScalarSlot => (RefCellBorrowResultAction::UseBorrowSlot, false),
        RefCellBorrowValueKind::PointerPayload
        | RefCellBorrowValueKind::SmartPointerPayload => {
            (RefCellBorrowResultAction::LoadPointerPayload, true)
        }
    }
}

/// lowering_rules.zig:3875 `planRefCellBorrowRuntimeGuard`: the
/// status register is released on both arms; a conflict panics
/// with code 107.
pub struct RefCellBorrowRuntimeGuardPlan {
    pub release_status_on_conflict: bool,
    pub conflict_panic_code: u32,
    pub release_status_on_success: bool,
}

pub fn plan_refcell_borrow_runtime_guard() -> RefCellBorrowRuntimeGuardPlan {
    RefCellBorrowRuntimeGuardPlan {
        release_status_on_conflict: true,
        conflict_panic_code: 107,
        release_status_on_success: true,
    }
}

/// lowering_rules.zig:3883 `planRefCellBorrowHandleRegistration`:
/// the receiver register is NOT tracked as an owner temp (the
/// cell keeps its own cleanup entry).
pub fn plan_refcell_borrow_handle_registration_track_receiver_owner_temp() -> bool {
    false
}

/// lowering_rules.zig `RefCellHandleReleasePlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefCellHandleReleasePlan {
    pub release_dynamic_borrow: bool,
    pub consume_handle_value: bool,
    pub release_owner_temps: bool,
}

/// lowering_rules.zig:3801 `planRefCellHandleRelease`.
pub fn plan_refcell_handle_release(has_owner_temps: bool) -> RefCellHandleReleasePlan {
    RefCellHandleReleasePlan {
        release_dynamic_borrow: true,
        consume_handle_value: true,
        release_owner_temps: has_owner_temps,
    }
}

/// lowering_rules.zig:3808 `planRefCellHandleCellRelease`:
/// releasing a cell register first settles every borrow handle
/// taken from it — except the handle being released itself.
pub fn plan_refcell_handle_cell_release(
    handle_cell_matches_release_target: bool,
    handle_is_release_target: bool,
) -> bool {
    if !handle_cell_matches_release_target {
        return false;
    }
    !handle_is_release_target
}

/// lowering_rules.zig:3562 `RefCellLoopStateMergeAction`: how
/// the emitter state is merged when a loop body finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefCellLoopStateMergeAction {
    RestorePreLoop,
}

impl RefCellLoopStateMergeAction {
    /// lowering_rules.zig:3565 `restoresPreLoop`.
    pub fn restores_pre_loop(self) -> bool {
        matches!(self, RefCellLoopStateMergeAction::RestorePreLoop)
    }
}

/// lowering_rules.zig:3776 `planRefCellLoopStateMerge`: the
/// merge is unconditional — a loop body may take or settle
/// borrows the pre-loop state did not have (and drop ones it
/// did), so after the body the RefCell handle table and the
/// borrow-source table are rebuilt from the pre-loop snapshot;
/// the back-edge re-enters the head with exactly the borrow
/// facts the first iteration started from.
pub fn plan_refcell_loop_state_merge() -> RefCellLoopStateMergeAction {
    RefCellLoopStateMergeAction::RestorePreLoop
}

/// Flattened fact accessor for the loop-state merge plan
/// (lowering_rules.zig:3776 plan + :3565 fact, combined the
/// way the loop emitters consume them).
pub fn plan_refcell_loop_state_merge_restores_pre_loop() -> bool {
    plan_refcell_loop_state_merge().restores_pre_loop()
}
