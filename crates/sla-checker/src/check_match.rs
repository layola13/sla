//! The `switch_expr` and `match_expr` arms (type_checker.zig:5464
//! and :5494), applying the multi-branch merge rule ported in
//! rules.rs (`plan_multi_branch_state_merge` +
//! `intersect_live_branch_value_states`) through
//! `Checker::merge_multi_branch_states`.
//!
//! Zig facts preserved verbatim:
//!   * `switch` checks its scrutinee and DISCARDS the type. The
//!     pre-FB-011 checker never looked at case patterns; FB-011
//!     (R-6 family F6) checks pattern expressions: identifiers
//!     named inside a pattern must resolve, and a literal pattern
//!     whose class differs from the scrutinee's is a TypeMismatch.
//!     Bare identifier patterns stay unchecked (they may name
//!     enum variants or constants). The result is void.
//!   * `match` on an `infer` scrutinee infers the scrutinee type
//!     from the FIRST case's pattern names with OR-conditions
//!     (enum name `Option` OR variant `Some`/`None` makes an
//!     Option, likewise Result) and then MUTATES the scrutinee
//!     binding's recorded type — see D-C16 / FB-014.
//!   * In both arms, only non-terminating cases contribute live
//!     end states; a live case without a tail type poisons the
//!     result to void (`has_value_cases`), and a terminating case's
//!     tail type is ignored entirely.

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    enum_name_matches_decl, node_key_of, option_inner_type, result_ok_type, void_ty,
};
use sla_ast::exprs::{MatchExpr, SwitchExpr};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

impl Checker {
    /// Mirrors the `switch_expr` arm (type_checker.zig:5464): every
    /// case body is checked from the pre-switch state; live
    /// (non-terminating) case end states are merged; result void.
    pub fn check_switch(&mut self, expr: &SwitchExpr) -> CheckResult<Type> {
        // The scrutinee type is computed and discarded in Zig
        // (`_ = val_ty;`): any type may be switched over.
        let scrut_ty = self.check_expr(&expr.val)?;

        let pre = self.scopes.save_states();
        let mut live_states = Vec::new();
        let mut live_bodies: Vec<&[Node]> = Vec::new();
        for case in &expr.cases {
            self.scopes.restore_states(&pre);
            self.check_switch_pattern(&case.pattern, &scrut_ty)?;
            self.check_block(&case.body)?;
            if !crate::check_program::block_terminates(&case.body) {
                live_states.push(self.scopes.save_states());
                live_bodies.push(&case.body);
            }
        }
        self.merge_multi_branch_states(&pre, &live_states, &live_bodies);
        Ok(void_ty())
    }

    /// FB-011 pattern checking for `switch` cases (R-6 family F6):
    ///   * the `default` pattern and bare identifier patterns are
    ///     not checked (a bare name may be an enum variant or a
    ///     constant pattern — neither is resolved here);
    ///   * any other pattern expression is fully checked, so an
    ///     undefined identifier inside it is an UndefinedVariable;
    ///   * a literal pattern whose class (string vs integer)
    ///     differs from the scrutinee's class is a TypeMismatch.
    fn check_switch_pattern(
        &mut self,
        pattern: &Node,
        scrut_ty: &Type,
    ) -> CheckResult<()> {
        if crate::types_util::is_switch_default_pattern(pattern) {
            return Ok(());
        }
        if matches!(pattern, Node::Identifier(_)) {
            return Ok(());
        }
        let pat_ty = self.check_expr(pattern)?;
        let pat_is_string = matches!(pattern, Node::Literal(sla_ast::exprs::Literal::StringVal(_)));
        let pat_is_int = matches!(pattern, Node::Literal(sla_ast::exprs::Literal::IntVal(_)));
        if pat_is_string || pat_is_int {
            let scrut_is_string = crate::types_util::is_string_type(scrut_ty)
                || matches!(scrut_ty, Type::Primitive(Primitive::RawPtr));
            if pat_is_string != scrut_is_string {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "TypeMismatch: switch pattern of type {} does not match scrutinee type {}",
                        crate::types_util::type_desc(&pat_ty),
                        crate::types_util::type_desc(scrut_ty)
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Mirrors the `match_expr` arm (type_checker.zig:5494) with its
    /// three paths: infer-from-pattern, Option/Result, plain enum.
    pub fn check_match(&mut self, expr: &MatchExpr) -> CheckResult<Type> {
        let mut val_ty = self.check_expr(&expr.val)?;

        if matches!(val_ty, Type::Infer) && !expr.cases.is_empty() {
            let first = &expr.cases[0].pattern;
            let inferred = if first.enum_name == "Option"
                || first.variant_name == "Some"
                || first.variant_name == "None"
            {
                Type::UserDefined(sla_ast::types::UserDefined {
                    name: "Option".to_string(),
                    generics: vec![Type::Infer],
                })
            } else if first.enum_name == "Result"
                || first.variant_name == "Ok"
                || first.variant_name == "Err"
            {
                Type::UserDefined(sla_ast::types::UserDefined {
                    name: "Result".to_string(),
                    generics: vec![Type::Infer, Type::Infer],
                })
            } else if self.enum_variants.contains_key(&first.enum_name) {
                Type::UserDefined(sla_ast::types::UserDefined {
                    name: first.enum_name.clone(),
                    generics: Vec::new(),
                })
            } else {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "match pattern names unknown enum `{}`",
                        first.enum_name
                    ),
                ));
            };
            // Zig rewrites the scrutinee binding's type in place
            // (`sym.ty = inferred`) and re-records the expression
            // type; the mutation outlives the match.
            if let Node::Identifier(name) = &*expr.val {
                if let Some(binding) = self.scopes.lookup_mut(name) {
                    binding.ty = inferred.clone();
                }
            }
            self.expr_types
                .insert(node_key_of(&expr.val), inferred.clone());
            val_ty = inferred;
        }

        if option_inner_type(&val_ty).is_some() || result_ok_type(&val_ty).is_some() {
            return self.check_match_option_result(expr, &val_ty);
        }
        self.check_match_enum(expr, &val_ty)
    }

    /// The Option/Result path: each case's single binding comes
    /// from `pattern_binding_type`; case result types must agree.
    fn check_match_option_result(
        &mut self,
        expr: &MatchExpr,
        val_ty: &Type,
    ) -> CheckResult<Type> {
        let pre = self.scopes.save_states();
        let mut live_states = Vec::new();
        let mut live_bodies: Vec<&[Node]> = Vec::new();
        let mut result_ty: Option<Type> = None;
        let mut has_value_cases = true;

        for case in &expr.cases {
            self.scopes.restore_states(&pre);
            self.scopes.push_frame();
            let case_result = (|| -> CheckResult<()> {
                if let Some(binding_ty) =
                    self.pattern_binding_type(&case.pattern, val_ty, "match")?
                {
                    // pattern_binding_type proved exactly one
                    // binding exists on this path (Zig indexes
                    // bindings[0] unguarded here).
                    self.define_symbol(&case.pattern.bindings[0], binding_ty, true)?;
                }
                if let Some(guard) = &case.guard {
                    self.check_match_guard(guard)?;
                }
                self.check_block(&case.body)
            })();
            let terminated = crate::check_program::block_terminates(&case.body);
            let case_ty = crate::check_program::block_tail_type(self, &case.body);
            self.scopes.pop_frame();
            case_result?;
            if !terminated {
                live_states.push(self.scopes.save_states());
                live_bodies.push(&case.body);
            }
            match case_ty {
                Some(ty) => {
                    if terminated {
                        // Terminating cases never contribute.
                    } else if let Some(existing) = &result_ty {
                        if !crate::types_util::types_equal(existing, &ty) {
                            return Err(Diagnostic::new(
                                ErrorKind::TypeMismatch,
                                "match cases have mismatched types",
                            ));
                        }
                    } else {
                        result_ty = Some(ty);
                    }
                }
                None => {
                    if !terminated {
                        has_value_cases = false;
                    }
                }
            }
        }

        self.merge_multi_branch_states(&pre, &live_states, &live_bodies);
        if has_value_cases {
            if let Some(ty) = result_ty {
                return Ok(ty);
            }
        }
        Ok(void_ty())
    }

    /// The plain-enum path: the scrutinee must be a user-defined
    /// enum type; patterns are validated against the declaration
    /// (name match, variant existence, binding arity).
    fn check_match_enum(
        &mut self,
        expr: &MatchExpr,
        val_ty: &Type,
    ) -> CheckResult<Type> {
        let decl_name = match val_ty {
            Type::UserDefined(u) => u.name.clone(),
            _ => {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    "match scrutinee is not an enum value",
                ))
            }
        };
        let variants = self
            .enum_variants
            .get(&decl_name)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::new(
                    ErrorKind::NotAStruct,
                    format!("NotAStruct: `{decl_name}` is not a known enum"),
                )
            })?;

        let pre = self.scopes.save_states();
        let mut live_states = Vec::new();
        let mut live_bodies: Vec<&[Node]> = Vec::new();
        let mut result_ty: Option<Type> = None;
        let mut has_value_cases = true;

        for case in &expr.cases {
            self.scopes.restore_states(&pre);
            if !enum_name_matches_decl(&case.pattern.enum_name, &decl_name) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "match pattern enum `{}` does not match scrutinee enum `{decl_name}`",
                        case.pattern.enum_name
                    ),
                ));
            }
            let variant = variants
                .iter()
                .find(|v| v.name == case.pattern.variant_name)
                .ok_or_else(|| {
                    Diagnostic::new(
                        ErrorKind::FieldNotFound,
                        format!(
                            "FieldNotFound: enum {decl_name} has no variant {}",
                            case.pattern.variant_name
                        ),
                    )
                })?;
            if case.pattern.bindings.len() != variant.fields.len() {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidArgsCount,
                    format!(
                        "match pattern {}::{} expects {} bindings, got {}",
                        case.pattern.enum_name,
                        case.pattern.variant_name,
                        variant.fields.len(),
                        case.pattern.bindings.len()
                    ),
                ));
            }
            let fields: Vec<Type> =
                variant.fields.iter().map(|(_, t)| t.clone()).collect();
            self.scopes.push_frame();
            let case_result = (|| -> CheckResult<()> {
                for (binding, field_ty) in
                    case.pattern.bindings.iter().zip(fields.iter())
                {
                    self.define_symbol(binding, field_ty.clone(), true)?;
                }
                if let Some(guard) = &case.guard {
                    self.check_match_guard(guard)?;
                }
                self.check_block(&case.body)
            })();
            let terminated = crate::check_program::block_terminates(&case.body);
            let case_ty = crate::check_program::block_tail_type(self, &case.body);
            self.scopes.pop_frame();
            case_result?;
            if !terminated {
                live_states.push(self.scopes.save_states());
                live_bodies.push(&case.body);
            }
            match case_ty {
                Some(ty) => {
                    if terminated {
                        // Terminating cases never contribute.
                    } else if let Some(existing) = &result_ty {
                        if !crate::types_util::types_equal(existing, &ty) {
                            return Err(Diagnostic::new(
                                ErrorKind::TypeMismatch,
                                "match cases have mismatched types",
                            ));
                        }
                    } else {
                        result_ty = Some(ty);
                    }
                }
                None => {
                    if !terminated {
                        has_value_cases = false;
                    }
                }
            }
        }

        self.merge_multi_branch_states(&pre, &live_states, &live_bodies);
        if has_value_cases && result_ty.is_some() {
            return Ok(result_ty.expect("checked is_some"));
        }
        Ok(void_ty())
    }

    /// Match guards accept exactly boolean / legacy integer
    /// primitives (type_checker.zig match arms); every other type
    /// is a TypeMismatch.
    fn check_match_guard(&mut self, guard: &Node) -> CheckResult<()> {
        let guard_ty = self.check_expr(guard)?;
        match guard_ty {
            Type::Primitive(Primitive::Boolean) | Type::Primitive(Primitive::Integer) => {
                Ok(())
            }
            _ => Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                "match guard must be boolean or integer",
            )),
        }
    }
}
