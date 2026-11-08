//! Signature argument checking split out of check_call.rs
//! for the 700-line file cap (body unchanged): arity (incl.
//! variadic floors), the var out-parameter rule, generic
//! substitution, and per-argument assignability.

use crate::checker::{Checker, FuncSig};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::MacroParamConsumption;
use crate::types_util::{
    raw_ptr_ty, type_desc, types_equal, types_structurally_equal, value_assignable_to,
    void_ty,
};
use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::Type;

impl Checker {
    /// Mirrors Zig `checkCallArgsAgainstSignature`
    /// (type_checker.zig:703): arity, capability-form discipline
    /// (`is_move` params require `^` arguments; capability arguments
    /// to plain parameters are rejected), borrow coercions, and the
    /// width-STRICT array element rule that coexists with the
    /// width-insensitive `typesEqual` (D-C8).
    pub fn check_call_args_against_sig(
        &mut self,
        name: &str,
        sig: &FuncSig,
        args: &[Node],
        call_generics: &[Type],
    ) -> CheckResult<Type> {
        if sig.params.len() != args.len() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidArgsCount,
                format!(
                    "InvalidArgsCount: `{name}` expects {} args, got {}",
                    sig.params.len(),
                    args.len()
                ),
            ));
        }
        // R-3 (var Phase-2): the `fill(var x)` argument shape is
        // an OUT-parameter use of a `var` parameter: after the call
        // the caller's binding is initialized (the callee writes
        // through it). Marked before the generic argument loop,
        // which would read the uninitialized binding as a value.
        if sig.params.len() == 1 && args.len() == 1 && sig.params[0].is_var {
            if let Node::VarStmt(v) = &args[0] {
                self.set_state(&v.name, crate::rules::ValueState::Active);
                return Ok(sig.ret_ty.clone());
            }
        }
        // R-3: an explicitly generic call (`id<Pos>(...)`) checked
        // against a generic function's signature must substitute
        // the signature's types by the call's generic arguments
        // first — the parameter types still mention the function's
        // own parameters (T, A, B) and can never types_equal their
        // instantiations. Non-generic calls and implicit cases
        // (param type already concrete) are untouched.
        let subst_params: Vec<crate::checker::ParamSig>;
        let sig_params: &[crate::checker::ParamSig] = if !call_generics.is_empty() {
            match self.func_generics.get(name) {
                Some(fn_params) if !fn_params.is_empty() => {
                    subst_params = sig
                        .params
                        .iter()
                        .map(|p| {
                            let mut p2 = p.clone();
                            p2.ty = crate::check_arena::substitute_generics(
                                &p.ty,
                                fn_params,
                                call_generics,
                            );
                            p2
                        })
                        .collect();
                    &subst_params
                }
                _ => &sig.params,
            }
        } else {
            &sig.params
        };
        // R-3: the RETURN type of an explicitly generic call is
        // likewise substituted (`make2<Pos, Vel>(...)` returns
        // Pair2<Pos, Vel>, never Pair2<A, B>) — without this the
        // caller compares / field-accesses the raw signature type.
        let subst_ret: Type;
        let ret_ty: &Type = if !call_generics.is_empty() {
            match self.func_generics.get(name) {
                Some(fn_params) if !fn_params.is_empty() => {
                    subst_ret = crate::check_arena::substitute_generics(
                        &sig.ret_ty,
                        fn_params,
                        call_generics,
                    );
                    &subst_ret
                }
                _ => &sig.ret_ty,
            }
        } else {
            &sig.ret_ty
        };
        for (param, arg) in sig_params.iter().zip(args.iter()) {
            let is_capability_arg =
                matches!(arg, Node::MoveExpr(_) | Node::BorrowExpr(_));
            if param.is_move && !matches!(arg, Node::MoveExpr(_)) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "TypeMismatch: parameter `{}` of `{name}` requires a move argument",
                        param.name
                    ),
                ));
            }
            if is_capability_arg
                && !param.is_borrow
                && !param.is_move
                && !matches!(param.ty, Type::Pointer(_))
                && !matches!(param.ty, Type::Borrow(_))
            {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "Call to {name} passes capability argument to plain parameter {}",
                        param.name
                    ),
                ));
            }
            let arg_ty = self.check_expr(arg)?;
            // R-3 borrow-origin gate: when this parameter is the
            // ANNOTATED ORIGIN parameter of the callee, the
            // argument must be a borrowable place; a non-place
            // argument is BorrowOriginNotABinding and precedes
            // the type comparison (the corpus judges the shape,
            // not the temporary's type).
            if let Some(sig_origin) = &sig.ret_borrow_origin {
                if sig_origin == &param.name
                    && crate::check_borrow_origin::place_root_strict(arg)
                        .is_none()
                {
                    return Err(Diagnostic::new(
                        ErrorKind::BorrowOriginNotABinding,
                        format!(
                            "BorrowOriginNotABinding: argument for origin parameter `{}` of `{name}` must be a binding",
                            param.name
                        ),
                    ));
                }
            }
            // A parameter whose TYPE is a borrow (`x: &T`, Zig
            // type_checker.zig checkCallArgsAgainstSignature's
            // `param.ty.* == .borrow` arm): the argument must be a
            // borrow of the same inner type. This arm precedes the
            // capability test in Zig; here it also precedes the
            // is_borrow/auto-borrow matching below, whose arg-side
            // borrow strip would otherwise compare the stripped
            // argument against the un-stripped parameter type and
            // reject every `x: &T` call (R-3 arena borrow corpus).
            let matched = if let Type::Borrow(param_inner) = &param.ty {
                match &arg_ty {
                    Type::Borrow(arg_inner) => {
                        types_equal(param_inner, arg_inner)
                    }
                    _ => crate::check_method::plain_call_arg_matches_pub(
                        self, &param.ty, arg, &arg_ty,
                    ),
                }
            } else if param.is_borrow {
                // Borrow parameters accept the bare value (auto-borrow)
                // or an explicit borrow of the parameter type.
                types_equal(&param.ty, &arg_ty)
                    || types_equal(
                        &Type::Borrow(Box::new(param.ty.clone())),
                        &arg_ty,
                    )
            } else if let Type::Borrow(inner) = &arg_ty {
                types_equal(&param.ty, inner)
            } else if let (Type::Array(a), Type::Array(b)) = (&param.ty, &arg_ty) {
                // Width-strict for array elements (Zig comment: reject
                // silent integer-width array mismatches).
                a.len == b.len && types_structurally_equal(&a.elem, &b.elem)
            } else {
                types_equal(&param.ty, &arg_ty)
            };
            if !matched {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "TypeMismatch: argument for parameter `{}` expects {}, actual {}",
                        param.name,
                        type_desc(&param.ty),
                        type_desc(&arg_ty)
                    ),
                ));
            }
            // R-6 (F5/F7): the UncloneableCopy preserved gate and
            // by-value argument consumption (check_call_consume.rs).
            self.check_call_arg_consumption(name, param, arg, &arg_ty)?;
        }
        // R-3: calling an `async fn` yields a future of its
        // return type (the `.await` arm unwraps it).
        if sig.is_async {
            return Ok(Type::Future(Box::new(ret_ty.clone())));
        }
        Ok(ret_ty.clone())
    }
}

#[cfg(test)]
mod tests_r3_expand_wiring {
    //! The expand_tuple slice's regression anchor at the call
    //! layer: a root source carrying an @expand_tuple directive
    //! must be expanded by the front-end BEFORE parsing (the
    //! checker here consumes exactly what pipeline.rs's
    //! expansion step produces). If the wiring is dropped, the
    //! raw directive reaches the parser and this test fails with
    //! a syntax error at the `@` — as it did when the wiring was
    //! once lost (see RESULT §17).
    use crate::checker::Checker;
    use sla_ast::node::Node;

    #[test]
    fn expand_tuple_root_source_expands_then_checks() {
        let src = "@expand_tuple(2, 2, T) {\nstruct Pair$N<$TYPES> {\n@each(T) {\n    value_$I: $T,\n}\n}\n}\nfn id(x: Pair2<i64, i64>) -> Pair2<i64, i64> { return x; }\nfn main() -> i32 { let p = Pair2<i64, i64> { value_0: 1, value_1: 2 }; let q = id(p); return q.value_0; }";
        let expanded = sla_parser::source_expand::expand(src)
            .expect("directive must expand");
        assert!(!expanded.contains("@expand_tuple"), "directive must be gone: {expanded}");
        let mut parser =
            sla_parser::Parser::with_options(&expanded, sla_parser::ParserOptions::default());
        let root = parser.parse_program().expect("expanded source must parse");
        let Node::Program(prog) = root else { panic!("shape") };
        let mut c = Checker::new();
        c.check_program(&prog).expect("expanded program must check");
    }

    #[test]
    fn expand_tuple_invalid_directive_is_loud() {
        let src = "@expand_tuple(x) {\nstruct P { a: i32 }\n}\nfn main() -> i32 { return 0; }";
        assert!(
            sla_parser::source_expand::expand(src).is_err(),
            "an invalid directive must fail as InvalidExpandTuple, never parse raw"
        );
    }
}
