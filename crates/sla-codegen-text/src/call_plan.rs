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
        // Worker bodies are generated from the spawn helper's
        // CLONED closure AST (thread_spawn.rs stores
        // `closure.clone()`), so node-address-keyed checker facts
        // miss for every node inside the body — including the
        // argument's type, which decides Copy-ness here. With no
        // type the planner assumes an ownership transfer and
        // emits `^reg` for a by-value parameter; the host rejects
        // that prefix for a Copy scalar (CapabilityMismatch on
        // the worker call). Captures are seeded as typed local
        // bindings (thread_spawn_emit.rs), so an identifier
        // argument falls back to its binding type — the same
        // fallback shape as the surface receiver classification
        // (surface.rs, RUST-S9-BARE-CLONE-CALL).
        let arg_ty = self.expr_type(arg).or_else(|| {
            if let Node::Identifier(name) = arg {
                self.local_binding_type_for_name(name)
            } else {
                None
            }
        });
        // A macro-call argument (PTR_BYTE_AT et al.) carries no
        // recorded type (or an Infer one), and the planner then
        // assumes an ownership transfer and emits `^reg` for a
        // by-value parameter — rejected for a Copy scalar
        // (CapabilityMismatch on proxy_lower_byte / url_hex_val).
        // The checker already proved the argument assignable to
        // the parameter, so when the argument type is missing or
        // unknown, the parameter's declared type decides
        // Copy-ness.
        let arg_is_copy = match arg_ty.as_ref() {
            Some(t) if !matches!(t, Type::Infer) => self.type_is_copy_value(t),
            _ => param
                .map(|p| self.type_is_copy_value(&p.ty))
                .unwrap_or(false),
        };
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
            // codegen.zig:10016: the checker's side table, or the
            // structural macro-inline fallback below.
            array_to_slice_borrow: self.check.call_arg_array_to_slice_borrow(arg)
                || self.macro_array_to_slice_borrow_arg(arg, param),
            dyn_borrow_trait_name: self.dyn_borrow_trait_name_for(param, arg_ty.as_ref()),
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

    /// codegen.zig:9848 `macroArrayToSliceBorrowArg`: inside a
    /// macro inline the checker never visited the body call, so
    /// the borrow-array -> slice coercion is recognized
    /// structurally — a borrow of an identifier whose macro-arg
    /// type is an array, passed to a borrow-of-slice parameter.
    pub(crate) fn macro_array_to_slice_borrow_arg(
        &self,
        arg: &Node,
        param: Option<&Param>,
    ) -> bool {
        if self.macro_inline_depth == 0 {
            return false;
        }
        let Node::BorrowExpr(b) = arg else {
            return false;
        };
        let Node::Identifier(name) = &*b.expr else {
            return false;
        };
        let Some(param) = param else {
            return false;
        };
        if (!param.is_borrow && !matches!(*param.ty, Type::Borrow(_)))
            || crate::place::slice_element_type(&param.ty).is_none()
        {
            return false;
        }
        let Some(inner_ty) = self.macro_arg_type_for_name(name) else {
            return false;
        };
        matches!(inner_ty, Type::Array(_))
    }

    /// The dyn-borrow coercion fact for one call argument
    /// (codegen.zig reads it from the checker's `dyn_borrow_args`
    /// side table at :14092). The Rust checker keeps no such
    /// table (D-C21) but accepts a dyn-borrow argument by a pure
    /// type predicate (check_call_sig.rs / check_method.rs):
    /// the parameter is a borrow of a `dyn Trait` type and the
    /// argument is either a dyn value of a trait extending it,
    /// or a borrow of a concrete type implementing it. Recompute
    /// exactly that predicate from the ORIGINAL declared
    /// parameter (before the plan_param borrow adjustment).
    pub(crate) fn dyn_borrow_trait_name_for(
        &self,
        param: Option<&Param>,
        arg_ty: Option<&Type>,
    ) -> Option<String> {
        let param = param?;
        if !param.is_borrow {
            return None;
        }
        let trait_name = sla_lowering::dyn_trait_name(&param.ty)?;
        let arg_ty = arg_ty?;
        if let Some(arg_trait) = sla_lowering::dyn_trait_name(arg_ty) {
            if self.check.trait_extends_trait(arg_trait, trait_name) {
                return Some(trait_name.to_string());
            }
            return None;
        }
        if let Type::Borrow(inner) = arg_ty {
            if self.check.type_implements_trait(inner, trait_name) {
                return Some(trait_name.to_string());
            }
        }
        None
    }
}
