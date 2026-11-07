//! E4 plans — the lowering_rules / control_flow_rules
//! decision family the E group consumes, ported line-for-line
//! (sla-lowering has not ported this family; the D3 precedent in
//! copy_facts.rs ports such base facts locally, marked for
//! re-homing). Split from release_meta.rs (E4) by the 700-line
//! rule; the Codegen-side operations stay there.

use sla_ast::Type;
use sla_lowering::abi_passes_as_pointer;

use crate::types::RefCellBorrowKind;


// ----------------------------------------------------------------
// lowering_rules plan family (local port; re-home when
// sla-lowering picks the family up)
// ----------------------------------------------------------------

/// Zig `ParamCleanupAction` (lowering_rules.zig:46).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamCleanupAction {
    Skip,
    MarkConsumed,
    Consume,
    Release,
}

impl ParamCleanupAction {
    pub fn skips(self) -> bool {
        self == ParamCleanupAction::Skip
    }
    pub fn marks_consumed(self) -> bool {
        self == ParamCleanupAction::MarkConsumed
    }
    pub fn consumes(self) -> bool {
        self == ParamCleanupAction::Consume
    }
    pub fn releases(self) -> bool {
        self == ParamCleanupAction::Release
    }
}

/// Zig `ParamCleanupCapability` (lowering_rules.zig).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamCleanupCapability {
    None,
    Raw,
    Borrow,
    ByValue,
    Other,
}

/// Zig `ParamCleanupFacts` (lowering_rules.zig:69).
#[derive(Debug, Clone, Copy, Default)]
pub struct ParamCleanupFacts {
    pub is_param: bool,
    pub capability: ParamCleanupCapability,
    pub has_type: bool,
    pub abi_is_ptr: bool,
    pub is_copy_value: bool,
    pub is_fn_ptr: bool,
}

impl Default for ParamCleanupCapability {
    fn default() -> Self {
        ParamCleanupCapability::None
    }
}

/// Zig `planParamCleanup` (lowering_rules.zig:78), verbatim
/// decision order — including the by-value-copy release arm
/// whose Zig comment records the MemoryLeak-1012 verifier
/// history.
pub fn plan_param_cleanup(facts: ParamCleanupFacts) -> ParamCleanupAction {
    if !facts.is_param {
        return ParamCleanupAction::Release;
    }
    match facts.capability {
        ParamCleanupCapability::None | ParamCleanupCapability::Raw => {
            return ParamCleanupAction::Skip;
        }
        ParamCleanupCapability::Borrow => return ParamCleanupAction::Release,
        ParamCleanupCapability::ByValue | ParamCleanupCapability::Other => {}
    }
    if !facts.has_type {
        return ParamCleanupAction::Skip;
    }
    if facts.capability == ParamCleanupCapability::ByValue && facts.abi_is_ptr {
        return ParamCleanupAction::Consume;
    }
    if facts.capability == ParamCleanupCapability::ByValue && facts.is_copy_value {
        return ParamCleanupAction::Release;
    }
    if facts.is_fn_ptr {
        return ParamCleanupAction::Consume;
    }
    if facts.is_copy_value {
        return ParamCleanupAction::Consume;
    }
    if facts.abi_is_ptr {
        return ParamCleanupAction::Consume;
    }
    ParamCleanupAction::Skip
}

/// Zig `control_flow_rules.BranchStateMergeAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchStateMergeAction {
    RestorePre,
    RestoreThen,
    RestoreElse,
    KeepCurrent,
}

impl BranchStateMergeAction {
    pub fn restores_pre(self) -> bool {
        self == BranchStateMergeAction::RestorePre
    }
    pub fn restores_then(self) -> bool {
        self == BranchStateMergeAction::RestoreThen
    }
    pub fn restores_else(self) -> bool {
        self == BranchStateMergeAction::RestoreElse
    }
}

/// Zig `control_flow_rules.planBranchStateMerge`.
pub fn plan_branch_state_merge(then_terminated: bool, else_terminated: bool) -> BranchStateMergeAction {
    if then_terminated && else_terminated {
        return BranchStateMergeAction::RestorePre;
    }
    if then_terminated {
        return BranchStateMergeAction::RestoreElse;
    }
    if else_terminated {
        return BranchStateMergeAction::RestoreThen;
    }
    BranchStateMergeAction::KeepCurrent
}

/// Zig `planRefCellBranchStateMerge` = `planBranchStateMerge`.
pub fn plan_refcell_branch_state_merge(
    then_terminated: bool,
    else_terminated: bool,
) -> BranchStateMergeAction {
    plan_branch_state_merge(then_terminated, else_terminated)
}

/// Zig `control_flow_rules.MultiBranchStateMergeAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiBranchStateMergeAction {
    RestorePre,
    RestoreSingle,
    IntersectLive,
}

impl MultiBranchStateMergeAction {
    pub fn restores_pre(self) -> bool {
        self == MultiBranchStateMergeAction::RestorePre
    }
    pub fn restores_single(self) -> bool {
        self == MultiBranchStateMergeAction::RestoreSingle
    }
    pub fn intersects_live(self) -> bool {
        self == MultiBranchStateMergeAction::IntersectLive
    }
}

/// Zig `control_flow_rules.planMultiBranchStateMerge`.
pub fn plan_multi_branch_state_merge(live_branch_count: usize) -> MultiBranchStateMergeAction {
    match live_branch_count {
        0 => MultiBranchStateMergeAction::RestorePre,
        1 => MultiBranchStateMergeAction::RestoreSingle,
        _ => MultiBranchStateMergeAction::IntersectLive,
    }
}

/// Zig `RefCellBranchHandleOwnerMergeAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefCellBranchHandleOwnerMergeAction {
    MergeDynamicOwner,
    KeepStaticOwner,
}

impl RefCellBranchHandleOwnerMergeAction {
    pub fn is_merge_dynamic_owner(self) -> bool {
        self == RefCellBranchHandleOwnerMergeAction::MergeDynamicOwner
    }
}

/// Zig `planRefCellBranchHandleOwnerMerge`.
pub fn plan_refcell_branch_handle_owner_merge(
    then_live: bool,
    else_live: bool,
    same_kind: bool,
    same_owner: bool,
) -> RefCellBranchHandleOwnerMergeAction {
    if then_live && else_live && same_kind && !same_owner {
        return RefCellBranchHandleOwnerMergeAction::MergeDynamicOwner;
    }
    RefCellBranchHandleOwnerMergeAction::KeepStaticOwner
}

/// Zig `RefCellCompanionRestorePlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefCellCompanionRestorePlan {
    pub track_loaded_cell_owner_temp: bool,
    pub release_companion_slot_after_restore: bool,
}

/// Zig `planRefCellCompanionRestore`: both flags always true.
pub fn plan_refcell_companion_restore() -> RefCellCompanionRestorePlan {
    RefCellCompanionRestorePlan {
        track_loaded_cell_owner_temp: true,
        release_companion_slot_after_restore: true,
    }
}

/// Zig `RefCellCompanionStoreCleanupPlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefCellCompanionStoreCleanupPlan {
    pub consume_handle_value: bool,
    pub release_owner_temps: bool,
    pub release_borrow_address_temps: bool,
    pub clear_non_owning_metadata: bool,
}

/// Zig `planRefCellCompanionStoreCleanup`.
pub fn plan_refcell_companion_store_cleanup(
    has_owner_temps: bool,
    has_borrow_address_temps: bool,
    has_non_owning_metadata: bool,
) -> RefCellCompanionStoreCleanupPlan {
    RefCellCompanionStoreCleanupPlan {
        consume_handle_value: true,
        release_owner_temps: has_owner_temps,
        release_borrow_address_temps: has_borrow_address_temps,
        clear_non_owning_metadata: has_non_owning_metadata,
    }
}

/// Zig `BorrowAddressTempPlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowAddressTempPlan {
    pub track_primary_temp: bool,
    pub track_extra_temps: bool,
    pub remember: bool,
}

/// Zig `planBorrowAddressTemps`.
pub fn plan_borrow_address_temps(has_primary_temp: bool, has_extra_temps: bool) -> BorrowAddressTempPlan {
    BorrowAddressTempPlan {
        track_primary_temp: has_primary_temp,
        track_extra_temps: has_extra_temps,
        remember: has_primary_temp || has_extra_temps,
    }
}

/// Zig `BorrowAddressTempTransferAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowAddressTempTransferAction {
    MoveBorrowAddressTemps,
    TransferValueState,
}

impl BorrowAddressTempTransferAction {
    pub fn moves_borrow_address_temps(self) -> bool {
        self == BorrowAddressTempTransferAction::MoveBorrowAddressTemps
    }
}

/// Zig `planBorrowAddressTempTransfer`.
pub fn plan_borrow_address_temp_transfer(
    source_has_borrow_address_temps: bool,
) -> BorrowAddressTempTransferAction {
    if source_has_borrow_address_temps {
        BorrowAddressTempTransferAction::MoveBorrowAddressTemps
    } else {
        BorrowAddressTempTransferAction::TransferValueState
    }
}

/// Zig `BorrowAddressTempReleasePlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowAddressTempReleasePlan {
    pub release_borrow_value: bool,
    pub release_source_temps: bool,
}

/// Zig `planBorrowAddressTempRelease`.
pub fn plan_borrow_address_temp_release(
    has_borrow_address_temps: bool,
) -> BorrowAddressTempReleasePlan {
    BorrowAddressTempReleasePlan {
        release_borrow_value: has_borrow_address_temps,
        release_source_temps: has_borrow_address_temps,
    }
}

/// Zig `RefCellHandleOwnerTransferAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefCellHandleOwnerTransferAction {
    RebindOwner,
    KeepOwner,
}

impl RefCellHandleOwnerTransferAction {
    pub fn rebinds_owner(self) -> bool {
        self == RefCellHandleOwnerTransferAction::RebindOwner
    }
}

/// Zig `planRefCellHandleOwnerTransfer`.
pub fn plan_refcell_handle_owner_transfer(
    handle_cell_matches_source: bool,
) -> RefCellHandleOwnerTransferAction {
    if handle_cell_matches_source {
        RefCellHandleOwnerTransferAction::RebindOwner
    } else {
        RefCellHandleOwnerTransferAction::KeepOwner
    }
}

/// Zig `RefCellHandleTransferAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefCellHandleTransferAction {
    MoveBorrowHandle,
    TransferValueState,
}

impl RefCellHandleTransferAction {
    pub fn moves_borrow_handle(self) -> bool {
        self == RefCellHandleTransferAction::MoveBorrowHandle
    }
}

/// Zig `planRefCellHandleTransfer`.
pub fn plan_refcell_handle_transfer(
    source_has_refcell_handle: bool,
) -> RefCellHandleTransferAction {
    if source_has_refcell_handle {
        RefCellHandleTransferAction::MoveBorrowHandle
    } else {
        RefCellHandleTransferAction::TransferValueState
    }
}

/// Zig `RefCellValueStateTransferPlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefCellValueStateTransferPlan {
    pub handle: RefCellHandleTransferAction,
    pub borrow_address_temps: BorrowAddressTempTransferAction,
}

/// Zig `planRefCellValueStateTransfer`.
pub fn plan_refcell_value_state_transfer(
    source_has_refcell_handle: bool,
    source_has_borrow_address_temps: bool,
) -> RefCellValueStateTransferPlan {
    RefCellValueStateTransferPlan {
        handle: plan_refcell_handle_transfer(source_has_refcell_handle),
        borrow_address_temps: plan_borrow_address_temp_transfer(source_has_borrow_address_temps),
    }
}

/// Zig `RefCellHandleReleasePlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefCellHandleReleasePlan {
    pub release_dynamic_borrow: bool,
    pub consume_handle_value: bool,
    pub release_owner_temps: bool,
}

/// Zig `planRefCellHandleRelease`: dynamic-borrow release and
/// handle consumption are unconditional; owner temps follow
/// the handle's payload.
pub fn plan_refcell_handle_release(has_owner_temps: bool) -> RefCellHandleReleasePlan {
    RefCellHandleReleasePlan {
        release_dynamic_borrow: true,
        consume_handle_value: true,
        release_owner_temps: has_owner_temps,
    }
}

/// Zig `RefCellHandleCellReleaseAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefCellHandleCellReleaseAction {
    ReleaseHandle,
    Skip,
}

impl RefCellHandleCellReleaseAction {
    pub fn should_release(self) -> bool {
        self == RefCellHandleCellReleaseAction::ReleaseHandle
    }
}

/// Zig `planRefCellHandleCellRelease`: a handle is released
/// for a cell only when its cell matches the release target
/// and the handle is not itself the target (both mismatches
/// are `skip`, exactly as in Zig).
pub fn plan_refcell_handle_cell_release(
    handle_cell_matches_release_target: bool,
    handle_is_release_target: bool,
) -> RefCellHandleCellReleaseAction {
    if !handle_cell_matches_release_target {
        return RefCellHandleCellReleaseAction::Skip;
    }
    if handle_is_release_target {
        return RefCellHandleCellReleaseAction::Skip;
    }
    RefCellHandleCellReleaseAction::ReleaseHandle
}

/// Zig `refCellBorrowReleaseMacroName`.
pub fn refcell_borrow_release_macro_name(kind: RefCellBorrowKind) -> &'static str {
    match kind {
        RefCellBorrowKind::Shared => "REFCELL_U64_RELEASE_SHARED",
        RefCellBorrowKind::Mutable => "REFCELL_U64_RELEASE_MUT",
    }
}

/// Zig `ResultSlotTransferPlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultSlotTransferPlan {
    pub transfers_value: bool,
    pub needs_refcell_companion: bool,
}

/// Zig `planResultSlotTransfer`: value transfer iff the type
/// passes as a pointer; the RefCell companion is needed iff
/// the type is a borrow.
pub fn plan_result_slot_transfer(ty: &Type) -> ResultSlotTransferPlan {
    ResultSlotTransferPlan {
        transfers_value: abi_passes_as_pointer(ty),
        needs_refcell_companion: matches!(ty, Type::Borrow(_)),
    }
}

/// Zig `ResultSlotStoreLifecycleAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultSlotStoreLifecycleAction {
    TransferValueState,
    ReleaseSource,
    KeepSource,
}

impl ResultSlotStoreLifecycleAction {
    pub fn releases_source(self) -> bool {
        self == ResultSlotStoreLifecycleAction::ReleaseSource
    }
    pub fn transfers_value_state(self) -> bool {
        self == ResultSlotStoreLifecycleAction::TransferValueState
    }
}

/// Zig `planResultSlotStoreLifecycle`.
pub fn plan_result_slot_store_lifecycle(
    plan: ResultSlotTransferPlan,
    source_needs_release: bool,
) -> ResultSlotStoreLifecycleAction {
    if plan.transfers_value {
        return ResultSlotStoreLifecycleAction::TransferValueState;
    }
    if source_needs_release {
        ResultSlotStoreLifecycleAction::ReleaseSource
    } else {
        ResultSlotStoreLifecycleAction::KeepSource
    }
}

/// Zig `ResultSlotLoadLifecycleAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultSlotLoadLifecycleAction {
    LoadValueState,
    NoValueState,
}

impl ResultSlotLoadLifecycleAction {
    pub fn loads_value_state(self) -> bool {
        self == ResultSlotLoadLifecycleAction::LoadValueState
    }
}

/// Zig `planResultSlotLoadLifecycle`.
pub fn plan_result_slot_load_lifecycle(
    plan: ResultSlotTransferPlan,
) -> ResultSlotLoadLifecycleAction {
    if plan.transfers_value {
        ResultSlotLoadLifecycleAction::LoadValueState
    } else {
        ResultSlotLoadLifecycleAction::NoValueState
    }
}

/// Zig `ResultSlotRefCellStoreAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultSlotRefCellStoreAction {
    StoreBorrowHandleCompanion,
    TransferValueState,
}

impl ResultSlotRefCellStoreAction {
    pub fn stores_borrow_handle_companion(self) -> bool {
        self == ResultSlotRefCellStoreAction::StoreBorrowHandleCompanion
    }
}

/// Zig `planResultSlotRefCellStore`.
pub fn plan_result_slot_refcell_store(
    plan: ResultSlotTransferPlan,
    source_has_refcell_handle: bool,
) -> ResultSlotRefCellStoreAction {
    if plan.transfers_value && source_has_refcell_handle {
        ResultSlotRefCellStoreAction::StoreBorrowHandleCompanion
    } else {
        ResultSlotRefCellStoreAction::TransferValueState
    }
}

/// Zig `ResultSlotRefCellLoadAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultSlotRefCellLoadAction {
    RestoreBorrowHandleCompanion,
    ReleaseEmptyCompanion,
    TransferValueState,
}

impl ResultSlotRefCellLoadAction {
    pub fn restores_borrow_handle_companion(self) -> bool {
        self == ResultSlotRefCellLoadAction::RestoreBorrowHandleCompanion
    }
    pub fn releases_empty_companion(self) -> bool {
        self == ResultSlotRefCellLoadAction::ReleaseEmptyCompanion
    }
}

/// Zig `planResultSlotRefCellLoad`.
pub fn plan_result_slot_refcell_load(
    plan: ResultSlotTransferPlan,
    slot_has_refcell_handle: bool,
    slot_has_refcell_companion: bool,
) -> ResultSlotRefCellLoadAction {
    if !plan.transfers_value {
        return ResultSlotRefCellLoadAction::TransferValueState;
    }
    if slot_has_refcell_handle {
        return ResultSlotRefCellLoadAction::RestoreBorrowHandleCompanion;
    }
    if slot_has_refcell_companion {
        return ResultSlotRefCellLoadAction::ReleaseEmptyCompanion;
    }
    ResultSlotRefCellLoadAction::TransferValueState
}
