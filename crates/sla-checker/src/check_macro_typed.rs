//! R-3 (J-3/J-4): typed macro parameters and the leading-output
//! expression form. Split out of check_macro.rs for the 700-line
//! file cap; the analyses proper stay there.

use crate::checker::Checker;
use crate::diag::CheckResult;
use sla_ast::decls::MacroDecl;
use sla_ast::types::Type;

/// R-3 (J-3/J-4): the leading-output name rule shared with
/// imported SA macros (`isLeadingOutputMacroParam`,
/// plugin_imported_macros.zig): a leading `out*` name, or one of
/// the listed names.
pub fn is_leading_output_param(name: &str) -> bool {
    name.starts_with("out")
        || matches!(
            name,
            "nonnull_ptr" | "type_id" | "any_ref" | "cursor" | "take" | "repeat"
        )
}

impl Checker {
    /// R-3 (J-3): check a user-macro call's arguments against the
    /// declared parameter types and judge the J-4 expression form.
    /// Returns the call's result type: `void` for the full-arity
    /// statement form, the leading output's declared type for the
    /// omitted-output expression form. Typed parameters are
    /// checked with the same value-assignability the function
    /// signature path uses; a mismatch is a TypeMismatch naming
    /// the macro, the parameter and both types.
    pub fn check_user_macro_typed_call(
        &mut self,
        mac: &MacroDecl,
        call: &sla_ast::exprs::CallExpr,
    ) -> CheckResult<Type> {
        use crate::diag::{Diagnostic, ErrorKind};
        let arity = mac.params.len();
        let typed = |i: usize| mac.param_tys.get(i).and_then(|t| t.as_ref());
        let check_args = |checker: &mut Self, offset: usize| -> CheckResult<()> {
            for (i, arg) in call.args.iter().enumerate() {
                let Some(want) = typed(i + offset) else {
                    continue;
                };
                let got = checker.expr_types.get(&crate::types_util::node_key_of(arg)).cloned();
                let Some(got) = got else { continue };
                if !crate::types_util::value_assignable_to(want, &got) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: macro `{}` argument for parameter `{}` expects {}, got {}",
                            mac.name,
                            mac.params[i + offset],
                            crate::types_util::type_desc(want),
                            crate::types_util::type_desc(&got),
                        ),
                    ));
                }
            }
            Ok(())
        };
        if call.args.len() == arity {
            check_args(self, 0)?;
            return Ok(crate::types_util::void_ty());
        }
        // J-4: the leading output may be omitted only when the
        // first parameter satisfies the shared name rule.
        if call.args.len() + 1 == arity
            && !mac.params.is_empty()
            && is_leading_output_param(&mac.params[0])
        {
            let Some(out_ty) = typed(0) else {
                return Err(Diagnostic::new(
                    ErrorKind::MacroExpressionOutputUntyped,
                    format!(
                        "MacroExpressionOutputUntyped: macro `{}` leading output parameter `{}` declares no type",
                        mac.name, mac.params[0]
                    ),
                ));
            };
            check_args(self, 1)?;
            return Ok(out_ty.clone());
        }
        Err(Diagnostic::new(
            ErrorKind::InvalidArgsCount,
            format!(
                "macro {} expects {} args, got {}",
                mac.name,
                arity,
                call.args.len()
            ),
        ))
    }

    /// R-3 (J-3): check a macro body standalone, with the declared
    /// parameter types visible (untyped parameters bind as
    /// `.infer`, as before). Runs once per macro at program-check
    /// time; bodies of untyped macros keep their historical
    /// behavior only insofar as `.infer` placeholders accepted
    /// them — a body that misuses a DECLARED type now fails here
    /// even if no call site exists yet.
    pub fn check_macro_body(&mut self, mac: &MacroDecl) -> CheckResult<()> {
        if mac.body.is_empty() || mac.param_tys.iter().all(|t| t.is_none()) {
            return Ok(());
        }
        let frame = self.scopes.push_frame();
        let saved_root = self.fn_root_depth;
        self.fn_root_depth = frame;
        let mut result = Ok(());
        for (i, param) in mac.params.iter().enumerate() {
            let ty = mac
                .param_tys
                .get(i)
                .and_then(|t| t.clone())
                .unwrap_or(Type::Infer);
            if let Err(e) = self.define_symbol(param, ty, false) {
                result = Err(e);
                break;
            }
        }
        if result.is_ok() {
            result = self.check_block(&mac.body);
        }
        self.scopes.pop_frame();
        self.fn_root_depth = saved_root;
        result
    }
}
