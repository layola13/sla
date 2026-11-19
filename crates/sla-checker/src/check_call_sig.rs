//! Signature argument checking split out of check_call.rs
//! for the 700-line file cap (body unchanged): arity (incl.
//! variadic floors), the var out-parameter rule, generic
//! substitution, and per-argument assignability.

use crate::checker::{Checker, FuncSig};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::MacroParamConsumption;
use crate::types_util::{
    is_string_type, node_key_of, raw_ptr_ty, type_desc, types_equal,
    types_structurally_equal, value_assignable_to, void_ty,
};
use sla_ast::exprs::{CallExpr, Literal};
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
        // Var Phase-2 argument forms (`fill(var x)`) are handled
        // per-position inside the generic argument loop below: a
        // var arg is a USE of the caller's slot (the callee reads
        // and writes through it), so the binding must already be
        // initialized — the out-parameter reading that used to sit
        // here contradicted the corpus negatives the shape comes
        // from (see LINE_VARPARAM_INTERIM fork adjudication).
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
        // View-types stage 1 (LINE-VIEWARGS-FIX): pairwise
        // field-place overlap validation for this call's arguments
        // (Zig `checkCallArgsAgainstSignature` runs
        // `checkCallArgPlaceOverlaps` before per-arg checking so
        // overlapping borrows are reported as borrow conflicts).
        // Classification uses the DECLARED parameters, as in Zig.
        self.check_call_arg_place_overlaps(
            name,
            &sig.params,
            &sig.param_may_write,
            args,
        )?;
        for (param, arg) in sig_params.iter().zip(args.iter()) {
            // Var Phase-2: a `var` argument (`fill(var x)`) passes
            // the caller's slot itself. It is only well-formed
            // against a `var` parameter, and it counts as a use of
            // the slot: the binding must already be initialized
            // (corpus test_error_var_param_phase2 — an uninitialized
            // var passed as a var argument is UseBeforeInit, exactly
            // like the plain-argument sibling
            // test_error_var_call_arg_before_init). After the call
            // the binding stays Active. Against a plain parameter
            // the form itself is the error (corpus
            // test_error_var_call_var_arg_phase2), reported as a
            // TypeMismatch like the capability-form mismatch below
            // instead of falling through to the slice trap.
            if let Node::VarStmt(v) = arg {
                if !param.is_var {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "Call to {name} passes var argument to plain parameter {}",
                            param.name
                        ),
                    ));
                }
                match self.lookup_state(&v.name) {
                    Some(crate::rules::ValueState::Uninitialized) => {
                        return Err(Diagnostic::new(
                            ErrorKind::UseBeforeInit,
                            format!("UseBeforeInit: `{}` is not initialized", v.name),
                        ));
                    }
                    Some(crate::rules::ValueState::Consumed) => {
                        return Err(Diagnostic::new(
                            ErrorKind::UseAfterMove,
                            format!(
                                "UseAfterMove: identifier `{}` was already consumed",
                                v.name
                            ),
                        ));
                    }
                    Some(crate::rules::ValueState::Active) => {}
                    None => {
                        return Err(Diagnostic::new(
                            ErrorKind::UndefinedVariable,
                            format!("UndefinedVariable: `{}`", v.name),
                        ));
                    }
                }
                if let Some(slot_ty) = self.lookup_ty(&v.name) {
                    if !types_equal(&slot_ty, &param.ty) {
                        return Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            format!(
                                "TypeMismatch: var argument `{}` of type {} does not match parameter `{}` of `{name}` of type {}",
                                v.name,
                                type_desc(&slot_ty),
                                param.name,
                                type_desc(&param.ty)
                            ),
                        ));
                    }
                }
                // The parser stamps the argument's VarStmt with a
                // placeholder type; record the parameter type so
                // codegen sees the slot's real type.
                self.expr_types.insert(node_key_of(arg), param.ty.clone());
                continue;
            }
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
            // R2b (stage 6): an array / repeat literal argument
            // takes its element type from the parameter — Zig
            // records the annotation for the whole node and the
            // emitter reads it back (codegen.zig:16365-16371).
            // Re-record the node's expr type as the parameter's
            // array type when the lengths agree and accept the
            // argument; non-literal array arguments keep the
            // width-strict rule below (a bare integer literal has
            // no width of its own to mismatch).
            let param_arr = match &param.ty {
                Type::Array(pa) => Some(pa.clone()),
                Type::Borrow(inner) => match &**inner {
                    Type::Array(pa) => Some(pa.clone()),
                    _ => None,
                },
                _ => None,
            };
            let literal_ctx_ty = match (arg, &param_arr) {
                (Node::ArrayLiteral(a), Some(pa))
                    if a.elements.len() == pa.len =>
                {
                    Some(Type::Array(pa.clone()))
                }
                (Node::RepeatArrayLiteral(r), Some(pa)) if r.len == pa.len => {
                    Some(Type::Array(pa.clone()))
                }
                _ => None,
            };
            if let Some(ty) = &literal_ctx_ty {
                self.expr_types.insert(node_key_of(arg), ty.clone());
            }
            let matched = if let Type::Borrow(param_inner) = &param.ty {
                match &arg_ty {
                    Type::Borrow(arg_inner) => {
                        types_equal(param_inner, arg_inner)
                            // A borrow EXPRESSION of an already-borrow
                            // binding (`&view` where `view: &[u8]`)
                            // reborrows the same view: Zig's shared
                            // plainCallArgMatches accepts it via
                            // unwrapBorrowForCallArg (typesEqual of
                            // the parameter against the unwrapped
                            // argument). The pointee comparison above
                            // alone compares slice vs borrow and
                            // rejects the corpus shape
                            // (test_unit_borrow_return_views).
                            || crate::check_method::plain_call_arg_matches_pub(
                                self, &param.ty, arg, &arg_ty,
                            )
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
                    // Dyn-borrow parameter (Zig
                    // checkCallArgsAgainstSignature's is_borrow +
                    // dynTraitName arm, type_checker.zig:4596):
                    // a borrow of a concrete type implementing
                    // the trait coerces to the dyn borrow, and a
                    // dyn argument of an extending trait coerces
                    // up. The method path (check_method.rs
                    // check_method_args) has this branch; the
                    // plain-call path never did, so
                    // `render(&sprite)` for `&item: dyn Draw`
                    // was a TypeMismatch here only
                    // (test_unit_dyn_borrow_arg). Zig's
                    // dyn_borrow_args side table has no analogue
                    // in this crate (D-C21), same as the method
                    // path.
                    || crate::types_util::dyn_trait_name(&param.ty).is_some_and(
                        |trait_name| {
                            if let Some(arg_trait) =
                                crate::types_util::dyn_trait_name(&arg_ty)
                            {
                                self.trait_extends_trait(arg_trait, trait_name)
                            } else {
                                match &arg_ty {
                                    Type::Borrow(inner) => self
                                        .type_implements_trait(inner, trait_name),
                                    _ => false,
                                }
                            }
                        },
                    )
            } else if let Type::Borrow(inner) = &arg_ty {
                types_equal(&param.ty, inner)
                    // A borrow EXPRESSION passed to a pointer
                    // parameter (`&a` for `p: *Handle`): Zig's
                    // shared plainCallArgMatches accepts it when
                    // the pointees match (type_checker.zig
                    // callMatchTypesEqual pointer/borrow arm); the
                    // method path (check_method.rs
                    // plain_call_arg_matches) has always had the
                    // case, but the plain-call path never consulted
                    // it, so the call was a TypeMismatch here only
                    // (test_unit_ptr_typed_eq).
                    || crate::check_method::plain_call_arg_matches_pub(
                        self, &param.ty, arg, &arg_ty,
                    )
            } else if let (Type::Array(a), Type::Array(b)) = (&param.ty, &arg_ty) {
                // Width-strict for array elements (Zig comment: reject
                // silent integer-width array mismatches).
                a.len == b.len && types_structurally_equal(&a.elem, &b.elem)
            } else {
                types_equal(&param.ty, &arg_ty)
                    // DEF-RUST-STRPTR-CHECKER: Zig's
                    // plainCallArgMatches string-literal arm
                    // (type_checker.zig:4634) — the method path
                    // (check_method.rs plain_call_arg_matches) has
                    // always had it; the plain-call path here never
                    // consulted it, so a literal passed to a String
                    // parameter was a TypeMismatch on this path only.
                    || (matches!(arg, Node::Literal(Literal::StringVal(_)))
                        && is_string_type(&param.ty))
            };
            let matched = matched || literal_ctx_ty.is_some();
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
