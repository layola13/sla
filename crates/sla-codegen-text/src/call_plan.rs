//! Call-argument planning, split out of call.rs for the
//! per-file line cap: Zig `genPlannedCallArg` (codegen.zig:9986)
//! — build the materialization input from the parameter and
//! argument facts, plan, and dispatch to call.rs's plan arms.

use sla_ast::{Node, Param, Type};
use sla_lowering::{
    abi_param_needs_borrow_arg, call_arg_is_copy_struct_value,
    plan_call_arg_materialization, value_arg_transfers_ownership_from_param,
    CallArgMaterializationInput, CallArgTarget,
};

use crate::call::LoweredCallArg;
use crate::codegen::Codegen;
use crate::Result;

impl<'c> Codegen<'c> {
    /// codegen.zig:9986 `genPlannedCallArg`.
    pub(crate) fn gen_planned_call_arg(
        &mut self,
        arg: &Node,
        param: Option<&Param>,
        receiver_style_auto_borrow: bool,
    ) -> Result<LoweredCallArg> {
        let arg_ty = self.expr_type(arg);
        let arg_is_copy = arg_ty
            .as_ref()
            .map(|t| self.type_is_copy_value(t))
            .unwrap_or(false);
        // A parameter whose TYPE is a borrow (`v: &Vec<T>`)
        // carries the borrow capability in the callee contract
        // (the declaration emitter renders it `&v: ptr`), so for
        // PLANNING an owned, not-yet-borrowed argument it is a
        // borrow parameter: the auto-borrow predicates key on
        // is_borrow alone and would otherwise plan a move
        // (`^v` at the call site, rejected with
        // CapabilityMismatch — using_ext_borrow_auto). An
        // argument that is already a borrow value keeps the
        // declared flag (no double borrow).
        let plan_param = param.cloned().map(|mut p| {
            if !matches!(arg_ty, Some(Type::Borrow(_)))
                && abi_param_needs_borrow_arg(p.is_borrow, &p.ty)
            {
                p.is_borrow = true;
            }
            p
        });
        let input = CallArgMaterializationInput {
            param: plan_param,
            arg_ty: arg_ty.clone(),
            arg_index: if receiver_style_auto_borrow { 0 } else { 1 },
            auto_borrow_receiver: receiver_style_auto_borrow,
            receiver_style_auto_borrow,
            statement_receiver_auto_borrow: false,
            abi_borrow_auto_borrow: param
                .map(|p| abi_param_needs_borrow_arg(p.is_borrow, &p.ty))
                .unwrap_or(false),
            array_to_slice_borrow: false,
            dyn_borrow_trait_name: None,
            copy_struct_value: call_arg_is_copy_struct_value(
                arg,
                param,
                param
                    .map(|p| self.check.type_is_copy_struct(&p.ty))
                    .unwrap_or(false),
            ),
            generated_fn_ptr_identifier: false,
            generated_scalar_const_identifier: false,
            local_fn_ptr_identifier: matches!(arg, Node::Identifier(_))
                && matches!(arg_ty, Some(Type::FnPtr(_))),
            // Zig `callArgIsShallowCopyValueCandidate` plus the
            // struct-layout gate: identifier argument, by-value
            // parameter, user-defined non-Copy non-borrow-like
            // shallow-copyable type with a struct declaration.
            // The raw fact alone would select the shallow plan
            // for scalar arguments (i32 pushes) too.
            shallow_copy_value: arg_ty
                .as_ref()
                .map(|t| {
                    matches!(arg, Node::Identifier(_))
                        && param
                            .map(|p| !p.is_borrow && !p.is_move)
                            .unwrap_or(false)
                        && matches!(t, Type::UserDefined(_))
                        && !self.check.type_is_copy_value(t)
                        && !sla_lowering::is_borrow_like_type(t)
                        && self.shallow_copy_call_arg_value(t, 0)
                        && match t {
                            Type::UserDefined(u) => {
                                crate::place::struct_decl_lookup(self.check, &u.name)
                                    .is_some()
                            }
                            _ => false,
                        }
                })
                .unwrap_or(false),
            value_arg_transfers_ownership: value_arg_transfers_ownership_from_param(
                param,
                arg_ty.as_ref(),
                arg_is_copy,
            ),
            target: CallArgTarget::SaText,
        };
        let plan = plan_call_arg_materialization(arg, &input);
        // The shallow-copy plan is actionable inside the
        // plain-record subset (copy.rs) and, since stage 3, for
        // any record the deep clone dispatcher (clone.rs) can
        // copy. Re-plan an argument without the shallow fact
        // only when neither applies — it takes the ordinary
        // Value (ownership-transfer) plan, the treatment this
        // emitter gave it before the shallow fact existed. If
        // the source binding is used after the call the host
        // traps loudly (UseAfterMove); it never silently
        // miscompiles.
        let plan = if plan.kind
            == sla_lowering::CallArgMaterializationKind::ShallowCopyPreservedValue
            && !arg_ty
                .as_ref()
                .map(|t| self.record_copyable(t) && !self.type_contains_string(t, 0))
                .unwrap_or(false)
        {
            let mut input_no_shallow = input.clone();
            input_no_shallow.shallow_copy_value = false;
            plan_call_arg_materialization(arg, &input_no_shallow)
        } else {
            plan
        };
        self.gen_call_arg_from_materialization_plan(arg, param, &plan)
    }
}
