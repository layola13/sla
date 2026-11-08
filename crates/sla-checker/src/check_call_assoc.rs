//! Trait-associated call checks split out of check_call.rs
//! for the 700-line file cap (body unchanged).

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
    /// The trait-target associated form (venue :8438):
    /// `Trait::method(recv, ...)` — args[0]'s concrete type must
    /// implement the trait; the declaring trait names the symbol.
    pub fn check_trait_associated_call(
        &mut self,
        trait_name: &str,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        let name = call.func_name.as_str();
        if call.args.is_empty() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidArgsCount,
                format!(
                    "Trait associated call {trait_name}::{name} needs an implementing value argument"
                ),
            ));
        }
        let recv_ty = self.check_expr(&call.args[0])?;
        let concrete = crate::types_util::concrete_type_name(&recv_ty)
            .ok_or_else(|| {
                Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "TypeMismatch: trait associated call receiver has no concrete type, actual {}",
                        crate::types_util::type_desc(&recv_ty)
                    ),
                )
            })?
            .to_string();
        let symbol = self
            .resolve_trait_method_symbol(
                &concrete,
                Some(trait_name),
                name,
                call.args.len(),
            )?
            .ok_or_else(|| {
                Diagnostic::new(
                    ErrorKind::UndefinedVariable,
                    format!(
                        "Type `{concrete}` does not implement trait method `{trait_name}::{name}`"
                    ),
                )
            })?;
        // Zig fetches the callee from funcs only on this path.
        let sig = self.funcs.get(&symbol).cloned().ok_or_else(|| {
            Diagnostic::new(
                ErrorKind::UndefinedVariable,
                format!("UndefinedVariable: `{symbol}`"),
            )
        })?;
        self.check_method_args(name, &sig, &call.args, false)
    }
}
