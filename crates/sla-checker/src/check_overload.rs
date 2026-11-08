//! Plain-function overload sets (R-6 families F2/F4), mirroring
//! the Zig checker's `func_overload_sets` discipline
//! (type_checker.zig:272, 3183-3540) and the monomorphizer's
//! GenericOverloadConflict rule.
//!
//! Registration (check_program.rs pass 2) groups same-name
//! declarations: an identical repeated signature is a
//! Redeclaration; distinct signatures form a set. A call resolves
//! by probing argument types under a scope-state snapshot (probe
//! consumptions roll back, exactly as Zig's
//! snapshotOverloadProbeStates), then matching each candidate with
//! the literal-adaptation rules of `overloadParamMatches`:
//! untyped literals adapt to any member of their class, so two
//! in-class members make the call AmbiguousCall. A generic member
//! in a multi-member set is a GenericOverloadConflict: generic
//! templates do not participate in overloading.

use crate::checker::{Checker, FuncSig, ParamSig};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{is_integer_type, is_string_type, type_desc, types_equal};
use sla_ast::decls::FuncDecl;
use sla_ast::exprs::{CallExpr, Literal};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

/// Build the registration signature of one declaration (the same
/// construction `register_func_as` performs).
pub(crate) fn sig_of_decl(func: &FuncDecl) -> FuncSig {
    FuncSig {
        params: func.params.iter().map(ParamSig::from).collect(),
        ret_ty: (*func.ret_ty).clone(),
        is_pub: func.is_pub,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
        is_async: func.is_async,
        is_variadic: func.is_variadic,
    }
}

/// Signature identity for the redeclaration test (Zig
/// `overloadClassSignaturesEqual`): parameter types and capability
/// flags. Parameter NAMES do not distinguish an overload class,
/// but integer WIDTH does — `pick(i32)` and `pick(i64)` are
/// distinct overloads, so the comparison uses the width-strict
/// structural equality, not the width-insensitive `types_equal`.
/// The return type is deliberately NOT compared: two declarations
/// that differ ONLY by return type are still a redeclaration, but
/// that shape is reported by the call site (the parameter-driven
/// resolution cannot distinguish them) — see
/// `check_return_only_redeclaration` in check_program.rs.
pub(crate) fn sigs_identical(a: &FuncSig, b: &FuncSig) -> bool {
    a.params.len() == b.params.len()
        && a.params.iter().zip(b.params.iter()).all(|(p, q)| {
            p.is_borrow == q.is_borrow
                && p.is_move == q.is_move
                && crate::types_util::types_structurally_equal(&p.ty, &q.ty)
        })
}

/// Render one candidate as Zig does in overload diagnostics:
/// `pick(i32) -> i32`.
pub(crate) fn render_sig(name: &str, sig: &FuncSig) -> String {
    let params = sig
        .params
        .iter()
        .map(|p| type_desc(&p.ty))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{name}({params}) -> {}", type_desc(&sig.ret_ty))
}

/// Overload candidate matching at the argument level. Only
/// UNTYPED literal arguments adapt to any member of their class
/// (that is what makes `pick(7)` ambiguous over i32/i64 members —
/// corpus test_error_overload_literal_ambiguous). A TYPED argument
/// (identifier, call result, any non-literal expression) matches
/// only the member whose parameter type is width-strictly equal
/// to its own type: resolution is "infer, then exact match on the
/// concrete argument types" (user overload ruling 2026-10-30), so
/// `pick(a)` with `a: i32` selects pick(i32) alone and must not be
/// reported ambiguous against pick(u64). The literal decision is
/// made on the argument NODE, never on the probed type — the probe
/// types every integer literal as i64, which would otherwise fake
/// an exact match for the i64 member.
pub(crate) fn overload_param_matches(
    param: &ParamSig,
    arg: &Node,
    arg_ty: &Type,
) -> bool {
    if param.is_move {
        return matches!(arg, Node::MoveExpr(_)) && types_equal(&param.ty, arg_ty);
    }
    // Untyped literals adapt to any member of their class.
    match arg {
        Node::Literal(Literal::IntVal(_)) => {
            return is_integer_type(&param.ty) || matches!(param.ty, Type::Infer);
        }
        Node::Literal(Literal::FloatVal(_)) => {
            return matches!(
                param.ty,
                Type::Primitive(Primitive::F32) | Type::Primitive(Primitive::F64)
            );
        }
        Node::Literal(Literal::StringVal(_)) => {
            return is_string_type(&param.ty);
        }
        _ => {}
    }
    // Typed argument: exact (width-strict) match only.
    if !matches!(arg, Node::Literal(_)) {
        let effective = match arg_ty {
            Type::Borrow(inner) => inner,
            other => other,
        };
        return crate::types_util::types_structurally_equal(&param.ty, effective);
    }
    if param.is_borrow {
        return match arg_ty {
            Type::Borrow(inner) => types_equal(&param.ty, inner),
            _ => types_equal(&param.ty, arg_ty),
        };
    }
    let effective = match arg_ty {
        Type::Borrow(inner) => inner,
        other => other,
    };
    types_equal(&param.ty, effective)
}

impl Checker {
    /// The return-only redeclaration rule (corpus
    /// test_error_overload_return_only): within one overload
    /// group, two members whose parameter lists are identical but
    /// whose return types differ are a Redeclaration — overloads
    /// may not differ by return type only, because resolution is
    /// driven by the arguments alone. Checked at registration;
    /// the group's call sites never see such a set.
    pub fn check_return_only_redeclaration(
        name: &str,
        sigs: &[FuncSig],
    ) -> Option<Diagnostic> {
        for i in 1..sigs.len() {
            for prev in &sigs[..i] {
                if sigs_identical(prev, &sigs[i])
                    && !crate::types_util::types_structurally_equal(
                        &prev.ret_ty,
                        &sigs[i].ret_ty,
                    )
                {
                    return Some(Diagnostic::new(
                        ErrorKind::Redeclaration,
                        format!(
                            "Redeclaration: function `{name}` is already defined; overloads may not differ by return type only ({} vs {})",
                            type_desc(&prev.ret_ty),
                            type_desc(&sigs[i].ret_ty)
                        ),
                    ));
                }
            }
        }
        None
    }

    /// Plain-call stage 5: overload sets first, then the single
    /// function / imported signature tables. `None` when the name
    /// resolves nowhere here (the caller falls through to the
    /// struct-constructor and later stages, as before).
    ///
    /// When ANY source carries a set for the name, the unified
    /// resolver (check_overload2.rs) owns the call — including the
    /// case where the set lives only in the imported or extern
    /// tables.
    pub fn resolve_user_call(
        &mut self,
        name: &str,
        call: &CallExpr,
    ) -> Option<CheckResult<Type>> {
        // R-3: an extern set for the name means the unified
        // resolver owns the call even when a plain-func set also
        // exists (mixed plain + extern-variadic sets resolve
        // across sources; the func-set-only path below cannot see
        // extern members and would misjudge their arity).
        if self.extern_overload_sets.contains_key(name) {
            return self.try_resolve_unified_call(name, call);
        }
        if let Some(set) = self.func_overload_sets.get(name).cloned() {
            return Some(self.resolve_overload_set_call(name, &set, call));
        }
        if self.imported_overload_sets.contains_key(name) {
            return self.try_resolve_unified_call(name, call);
        }
        // R-3: a name claimed by BOTH a plain function and a
        // singleton extern contract (the mixed plain +
        // extern-variadic shape) resolves through the unified
        // pool, where the extern competes instead of being
        // shadowed by the plain signature.
        if self.extern_funcs.contains_key(name)
            && (self.funcs.contains_key(name)
                || self.func_overload_sets.contains_key(name))
        {
            return self.try_resolve_unified_call(name, call);
        }
        let sig = self
            .funcs
            .get(name)
            .cloned()
            .or_else(|| self.imported_funcs.get(name).cloned());
        sig.map(|sig| self.check_call_args_against_sig(name, &sig, &call.args, &call.generics))
    }

    fn resolve_overload_set_call(
        &mut self,
        name: &str,
        set: &[FuncSig],
        call: &CallExpr,
    ) -> CheckResult<Type> {
        // A generic template must stand alone (the diagnostic is
        // produced by Zig's monomorphizer; this checker has none,
        // so the call site is where the set is first confronted).
        if self
            .func_generics
            .get(name)
            .map(|g| !g.is_empty())
            .unwrap_or(false)
        {
            return Err(Diagnostic::new(
                ErrorKind::GenericOverloadConflict,
                format!(
                    "Monomorphization Error: generic function `{name}` shares its name with another function declaration; generic templates do not participate in overloading, so a name with a generic template must stand alone"
                ),
            ));
        }
        // Probe the argument types once, under a snapshot; probing
        // must not consume (Zig rolls the probe states back before
        // the winning member's real argument check).
        let saved = self.scopes.save_states();
        let mut arg_tys: Vec<Type> = Vec::new();
        let mut probe_error: Option<Diagnostic> = None;
        for arg in &call.args {
            match self.check_expr(arg) {
                Ok(ty) => arg_tys.push(ty),
                Err(e) => {
                    probe_error = Some(e);
                    break;
                }
            }
        }
        self.scopes.restore_states(&saved);
        if let Some(e) = probe_error {
            return Err(e);
        }
        let mut winners: Vec<&FuncSig> = Vec::new();
        for sig in set {
            if sig.params.len() != call.args.len() {
                continue;
            }
            let matched = sig
                .params
                .iter()
                .zip(call.args.iter())
                .zip(arg_tys.iter())
                .all(|((p, a), t)| overload_param_matches(p, a, t));
            if matched {
                winners.push(sig);
            }
        }
        match winners.len() {
            0 => Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "no overload of `{name}` matches the argument types ({}); candidates: {}",
                    arg_tys
                        .iter()
                        .map(|t| type_desc(t))
                        .collect::<Vec<_>>()
                        .join(", "),
                    set.iter()
                        .map(|s| render_sig(name, s))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            )),
            1 => self.check_call_args_against_sig(name, winners[0], &call.args, &call.generics),
            _ => Err(Diagnostic::new(
                ErrorKind::AmbiguousCall,
                format!(
                    "AmbiguousCall: call to `{name}` matches multiple overloads: {}",
                    winners
                        .iter()
                        .map(|s| render_sig(name, s))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            )),
        }
    }

    /// A bare identifier reference to an overload set cannot choose
    /// a member (Zig identifier arm, type_checker.zig:7425).
    pub fn check_overload_set_reference(&self, name: &str) -> Option<Diagnostic> {
        let set = self.func_overload_sets.get(name)?;
        Some(Diagnostic::new(
            ErrorKind::AmbiguousCall,
            format!(
                "AmbiguousCall: `{name}` is an overload set and a bare function reference cannot choose a member; candidates: {}; call it with arguments, or reference one member by its member symbol (e.g. `{name}`)",
                set.iter()
                    .map(|s| render_sig(name, s))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        ))
    }
}
