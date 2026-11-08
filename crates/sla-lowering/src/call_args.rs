//! Call-argument materialization planning, mirrored from
//! lowering_rules.zig (lines 3631-3740, 5911-5923, 6210-6560). The
//! emitters ask for a plan per argument and then execute it; all
//! ownership facts that need the checker (Copy-ness, borrow-likeness)
//! are supplied by the emitter through the input struct, exactly like
//! the Zig original.

use sla_ast::{Node, Param, Type};

use crate::abi::{by_value_raw_pointer_param, is_borrow_like_type};
use crate::release_facts::call_arg_needs_release;

// ---------------------------------------------------------------------
// Small lifecycle plans used by the emitters' cleanup code
// ---------------------------------------------------------------------

/// lowering_rules.zig:3631 `RefCellCallArgLifecycleAction`
/// (verified verbatim against the Zig body at 3631-3635: keep when
/// not releasing after the call, release the borrow handle when the
/// candidate carries one, otherwise release the value; both release
/// variants make `shouldRelease` true — the emitter's release path
/// itself dispatches borrow handles to their EXPAND release).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefCellCallArgLifecycleAction {
    Keep,
    ReleaseBorrowHandle,
    ReleaseValue,
}

impl RefCellCallArgLifecycleAction {
    pub fn should_release(self) -> bool {
        !matches!(self, Self::Keep)
    }
}

/// lowering_rules.zig:3631 `planRefCellCallArgLifecycle`.
pub fn plan_refcell_call_arg_lifecycle(
    release_after_call: bool,
    carries_refcell_borrow_handle: bool,
) -> RefCellCallArgLifecycleAction {
    if !release_after_call {
        return RefCellCallArgLifecycleAction::Keep;
    }
    if carries_refcell_borrow_handle {
        return RefCellCallArgLifecycleAction::ReleaseBorrowHandle;
    }
    RefCellCallArgLifecycleAction::ReleaseValue
}

/// lowering_rules.zig `BorrowAddressTempReleasePlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowAddressTempReleasePlan {
    pub release_borrow_value: bool,
    pub release_source_temps: bool,
}

/// lowering_rules.zig:3708 `planBorrowAddressTempRelease`.
pub fn plan_borrow_address_temp_release(
    has_borrow_address_temps: bool,
) -> BorrowAddressTempReleasePlan {
    BorrowAddressTempReleasePlan {
        release_borrow_value: has_borrow_address_temps,
        release_source_temps: has_borrow_address_temps,
    }
}

/// lowering_rules.zig `StackSlotIdentifierCallArgTempAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackSlotIdentifierCallArgTempAction {
    Keep,
    ReleaseTemp,
    ConsumeTemp,
}

impl StackSlotIdentifierCallArgTempAction {
    pub fn is_keep(self) -> bool {
        self == StackSlotIdentifierCallArgTempAction::Keep
    }
    pub fn releases_temp(self) -> bool {
        self == StackSlotIdentifierCallArgTempAction::ReleaseTemp
    }
    pub fn consumes_temp(self) -> bool {
        self == StackSlotIdentifierCallArgTempAction::ConsumeTemp
    }
}

/// lowering_rules.zig:3637 `planStackSlotIdentifierCallArgTemp`.
pub fn plan_stack_slot_identifier_call_arg_temp(
    param: Option<&Param>,
    is_identifier_stack_slot_temp: bool,
) -> StackSlotIdentifierCallArgTempAction {
    if !is_identifier_stack_slot_temp {
        return StackSlotIdentifierCallArgTempAction::Keep;
    }
    let target_param = match param {
        None => return StackSlotIdentifierCallArgTempAction::ReleaseTemp,
        Some(p) => p,
    };
    if by_value_raw_pointer_param(target_param) {
        return StackSlotIdentifierCallArgTempAction::ConsumeTemp;
    }
    StackSlotIdentifierCallArgTempAction::ReleaseTemp
}

// ---------------------------------------------------------------------
// Auto-borrow predicates (lowering_rules.zig:6210-6245)
// ---------------------------------------------------------------------

/// lowering_rules.zig:6210 `shouldAutoBorrowResolvedArg`.
pub fn should_auto_borrow_resolved_arg(
    param: &Param,
    arg: &Node,
    arg_ty: &Type,
    arg_index: usize,
    auto_borrow_receiver: bool,
) -> bool {
    if !param.is_borrow || matches!(arg, Node::BorrowExpr(_)) {
        return false;
    }
    (auto_borrow_receiver && arg_index == 0) || matches!(arg_ty, Type::Borrow(_))
}

/// lowering_rules.zig:6221 `shouldAutoBorrowReceiverArg`. Mirrored
/// as-is, including the redundant inner branch (see divergences log).
pub fn should_auto_borrow_receiver_arg(param: &Param, arg: &Node, arg_ty: &Type) -> bool {
    if !param.is_borrow || matches!(arg, Node::BorrowExpr(_)) {
        return false;
    }
    if matches!(arg_ty, Type::Borrow(_)) {
        return true;
    }
    true
}

/// lowering_rules.zig:6227 `shouldAutoBorrowStatementReceiverArg`.
pub fn should_auto_borrow_statement_receiver_arg(param: &Param, arg: &Node, arg_ty: &Type) -> bool {
    if !param.is_borrow || matches!(arg, Node::BorrowExpr(_)) {
        return false;
    }
    !matches!(arg_ty, Type::Borrow(_))
}

/// lowering_rules.zig:6245 `callArgUsesRawPointerStringLiteralValue`.
pub fn call_arg_uses_raw_pointer_string_literal_value(arg: &Node, param: &Param) -> bool {
    if !matches!(arg, Node::Literal(sla_ast::Literal::StringVal(_))) {
        return false;
    }
    by_value_raw_pointer_param(param)
}

/// lowering_rules.zig:6252 `callArgIsCopyStructValue`.
pub fn call_arg_is_copy_struct_value(
    arg: &Node,
    param: Option<&Param>,
    param_is_copy_struct: bool,
) -> bool {
    let target = match param {
        None => return false,
        Some(p) => p,
    };
    if target.is_borrow || target.is_move {
        return false;
    }
    if !matches!(arg, Node::Identifier(_)) {
        return false;
    }
    param_is_copy_struct
}

// ---------------------------------------------------------------------
// Value-arg ownership (lowering_rules.zig:6530-6570)
// ---------------------------------------------------------------------

/// lowering_rules.zig `ValueArgOwnershipFacts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueArgOwnershipFacts {
    pub param_is_present: bool,
    pub param_is_borrow: bool,
    pub param_is_move: bool,
    pub param_is_by_value_raw_pointer: bool,
    pub arg_is_borrow_like: bool,
    pub arg_is_copy_value: bool,
}

/// lowering_rules.zig:6540 `planValueArgTransfersOwnership`.
pub fn plan_value_arg_transfers_ownership(facts: ValueArgOwnershipFacts) -> bool {
    if !facts.param_is_present {
        return false;
    }
    if facts.param_is_borrow || facts.param_is_move {
        return false;
    }
    if facts.param_is_by_value_raw_pointer {
        return false;
    }
    if facts.arg_is_borrow_like {
        return false;
    }
    !facts.arg_is_copy_value
}

/// lowering_rules.zig:6553 `valueArgTransfersOwnershipFromParam`.
pub fn value_arg_transfers_ownership_from_param(
    param: Option<&Param>,
    arg_ty: Option<&Type>,
    arg_is_copy_value: bool,
) -> bool {
    let target = match param {
        None => {
            return plan_value_arg_transfers_ownership(ValueArgOwnershipFacts {
                param_is_present: false,
                param_is_borrow: false,
                param_is_move: false,
                param_is_by_value_raw_pointer: false,
                arg_is_borrow_like: false,
                arg_is_copy_value: false,
            })
        }
        Some(p) => p,
    };
    let ty = arg_ty.unwrap_or(&target.ty);
    plan_value_arg_transfers_ownership(ValueArgOwnershipFacts {
        param_is_present: true,
        param_is_borrow: target.is_borrow,
        param_is_move: target.is_move,
        param_is_by_value_raw_pointer: by_value_raw_pointer_param(target),
        arg_is_borrow_like: is_borrow_like_type(ty),
        arg_is_copy_value,
    })
}

// ---------------------------------------------------------------------
// Materialization plan (lowering_rules.zig:6434-6492)
// ---------------------------------------------------------------------

/// Which backend the plan is for (lowering_rules.zig's input `target`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CallArgTarget {
    #[default]
    SaText,
    DirectSab,
}

/// lowering_rules.zig `CallArgMaterializationPlan` kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallArgMaterializationKind {
    RawPointerStringLiteral,
    ArrayToSliceBorrow,
    DynBorrow,
    AutoBorrow,
    CopyStructValue,
    GeneratedFnPtrValueSlot,
    BorrowLocalFnPtrValue,
    ShallowCopyPreservedValue,
    Value,
}

/// lowering_rules.zig `CallArgMaterializationPlan`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArgMaterializationPlan {
    pub kind: CallArgMaterializationKind,
    pub release_after_call: bool,
    pub transfers_ownership: bool,
    pub dyn_borrow_trait_name: Option<String>,
}

impl CallArgMaterializationPlan {
    pub fn is_raw_pointer_string_literal(&self) -> bool {
        self.kind == CallArgMaterializationKind::RawPointerStringLiteral
    }
    pub fn is_array_to_slice_borrow(&self) -> bool {
        self.kind == CallArgMaterializationKind::ArrayToSliceBorrow
    }
    pub fn is_dyn_borrow(&self) -> bool {
        self.kind == CallArgMaterializationKind::DynBorrow
    }
    pub fn is_auto_borrow(&self) -> bool {
        self.kind == CallArgMaterializationKind::AutoBorrow
    }
    pub fn is_copy_struct_value(&self) -> bool {
        self.kind == CallArgMaterializationKind::CopyStructValue
    }
    pub fn is_shallow_copy_preserved_value(&self) -> bool {
        self.kind == CallArgMaterializationKind::ShallowCopyPreservedValue
    }
    pub fn is_generated_fn_ptr_value_slot(&self) -> bool {
        self.kind == CallArgMaterializationKind::GeneratedFnPtrValueSlot
    }
    pub fn is_borrow_local_fn_ptr_value(&self) -> bool {
        self.kind == CallArgMaterializationKind::BorrowLocalFnPtrValue
    }
    pub fn is_value(&self) -> bool {
        self.kind == CallArgMaterializationKind::Value
    }
}

/// lowering_rules.zig `CallArgMaterializationInput`. Facts the emitter
/// owns (checker tables, later-use scans) arrive as plain values.
#[derive(Debug, Clone, Default)]
pub struct CallArgMaterializationInput {
    pub param: Option<Param>,
    pub arg_ty: Option<Type>,
    pub arg_index: usize,
    pub auto_borrow_receiver: bool,
    pub receiver_style_auto_borrow: bool,
    pub statement_receiver_auto_borrow: bool,
    pub abi_borrow_auto_borrow: bool,
    pub array_to_slice_borrow: bool,
    pub dyn_borrow_trait_name: Option<String>,
    pub copy_struct_value: bool,
    pub generated_fn_ptr_identifier: bool,
    pub generated_scalar_const_identifier: bool,
    pub local_fn_ptr_identifier: bool,
    pub shallow_copy_value: bool,
    pub value_arg_transfers_ownership: bool,
    pub target: CallArgTarget,
}

fn plan(kind: CallArgMaterializationKind, release_after_call: bool) -> CallArgMaterializationPlan {
    CallArgMaterializationPlan {
        kind,
        release_after_call,
        transfers_ownership: false,
        dyn_borrow_trait_name: None,
    }
}

/// lowering_rules.zig:6434 `planCallArgMaterialization`.
pub fn plan_call_arg_materialization(
    arg: &Node,
    input: &CallArgMaterializationInput,
) -> CallArgMaterializationPlan {
    if let Some(param) = &input.param {
        if call_arg_uses_raw_pointer_string_literal_value(arg, param) {
            return plan(CallArgMaterializationKind::RawPointerStringLiteral, true);
        }
    }
    if input.array_to_slice_borrow {
        return plan(
            CallArgMaterializationKind::ArrayToSliceBorrow,
            call_arg_needs_release(arg),
        );
    }
    if let Some(trait_name) = &input.dyn_borrow_trait_name {
        let mut p = plan(CallArgMaterializationKind::DynBorrow, false);
        p.dyn_borrow_trait_name = Some(trait_name.clone());
        return p;
    }
    if let (Some(param), Some(arg_ty)) = (&input.param, &input.arg_ty) {
        if input.statement_receiver_auto_borrow {
            if should_auto_borrow_statement_receiver_arg(param, arg, arg_ty) {
                return plan(
                    CallArgMaterializationKind::AutoBorrow,
                    call_arg_needs_release(arg) || input.generated_scalar_const_identifier,
                );
            }
        } else if input.receiver_style_auto_borrow {
            if should_auto_borrow_receiver_arg(param, arg, arg_ty) {
                return plan(
                    CallArgMaterializationKind::AutoBorrow,
                    call_arg_needs_release(arg) || input.generated_scalar_const_identifier,
                );
            }
        } else if input.abi_borrow_auto_borrow && param.is_borrow && !matches!(arg, Node::BorrowExpr(_)) {
            return plan(
                CallArgMaterializationKind::AutoBorrow,
                call_arg_needs_release(arg) || input.generated_scalar_const_identifier,
            );
        } else if should_auto_borrow_resolved_arg(
            param,
            arg,
            arg_ty,
            input.arg_index,
            input.auto_borrow_receiver,
        ) {
            return plan(
                CallArgMaterializationKind::AutoBorrow,
                call_arg_needs_release(arg) || input.generated_scalar_const_identifier,
            );
        }
    }
    if input.copy_struct_value {
        return plan(CallArgMaterializationKind::CopyStructValue, true);
    }
    if input.target == CallArgTarget::DirectSab {
        if input.generated_fn_ptr_identifier {
            return plan(CallArgMaterializationKind::GeneratedFnPtrValueSlot, true);
        }
        if input.local_fn_ptr_identifier {
            return plan(CallArgMaterializationKind::BorrowLocalFnPtrValue, false);
        }
    }
    if input.shallow_copy_value {
        return plan(CallArgMaterializationKind::ShallowCopyPreservedValue, false);
    }
    CallArgMaterializationPlan {
        kind: CallArgMaterializationKind::Value,
        release_after_call: !input.value_arg_transfers_ownership
            && (call_arg_needs_release(arg)
                || input.generated_fn_ptr_identifier
                || input.generated_scalar_const_identifier),
        transfers_ownership: input.value_arg_transfers_ownership,
        dyn_borrow_trait_name: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{BinaryExpr, BinaryOp, Literal, Primitive};

    fn i32_param() -> Param {
        Param {
            is_var: false,
            name: "x".to_string(),
            ty: Box::new(Type::Primitive(Primitive::I32)),
            is_borrow: false,
            is_move: false,
            fallible_sugar: false,
        }
    }

    #[test]
    fn plain_value_args() {
        // Slice shape: by-value i32 param, literal / identifier / binary args.
        let lit = Node::Literal(Literal::IntVal(5));
        let input = CallArgMaterializationInput {
            param: Some(i32_param()),
            arg_ty: Some(Type::Primitive(Primitive::I32)),
            ..Default::default()
        };
        let p = plan_call_arg_materialization(&lit, &input);
        assert!(p.is_value());
        assert!(p.release_after_call);
        assert!(!p.transfers_ownership);

        let id = Node::Identifier("a".to_string());
        let p = plan_call_arg_materialization(&id, &input);
        assert!(p.is_value());
        assert!(!p.release_after_call);

        let bin = Node::BinaryExpr(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(id.clone()),
            right: Box::new(lit.clone()),
        });
        let p = plan_call_arg_materialization(&bin, &input);
        assert!(p.is_value());
        assert!(p.release_after_call);
    }

    #[test]
    fn ownership_transfer_facts() {
        // Non-Copy by-value arg transfers; Copy arg does not.
        assert!(value_arg_transfers_ownership_from_param(
            Some(&i32_param()),
            Some(&Type::UserDefined(sla_ast::UserDefined { name: "Vec".into(), generics: vec![] })),
            false,
        ));
        assert!(!value_arg_transfers_ownership_from_param(
            Some(&i32_param()),
            Some(&Type::Primitive(Primitive::I32)),
            true,
        ));
    }
}
