//! Header-less extern contract call path (R-6 follow-up).
//!
//! Contracts registered programmatically without a parsed parameter
//! list (hand-built `ExternSig`s in unit tests, and the legacy
//! `register_extern_func` surface) have no spellings to validate
//! against, so there is nothing to check the arity or argument types
//! against: the historical FFI bypass checks each argument
//! expression, consumes any move-marked positions, and materializes
//! the ABI return type. Contracts WITH spellings go through
//! `check_extern_contract_call` in check_overload2 instead, which
//! enforces arity, the closure-to-`ptr` rejection, and move
//! contracts per Zig `checkExternContractCall` (staging :3387).

use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::Type;

use crate::checker::{extern_ret_type, Checker, ExternSig};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};

impl Checker {
    /// FFI bypass for header-less extern contracts.
    pub fn check_extern_headerless_call(
        &mut self,
        name: &str,
        ext: &ExternSig,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        // R-3: a variadic contract with zero FIXED parameters
        // lands here (its params list is empty after the va_list
        // marker is excluded); calls below the fixed floor are an
        // arity error, not a silent bypass.
        if ext.is_variadic && call.args.len() < ext.params.len() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidArgsCount,
                format!(
                    "call to variadic function {name} needs at least {} arguments ({} fixed), got {}",
                    ext.params.len(),
                    ext.params.len(),
                    call.args.len()
                ),
            ));
        }
        for arg in &call.args {
            self.check_expr(arg)?;
        }
        for (i, &mv) in ext.move_params.iter().enumerate() {
            let Some(arg) = call.args.get(i) else { continue };
            if mv && !matches!(arg, Node::MoveExpr(_)) {
                if let Node::Identifier(arg_name) = arg {
                    match self.lookup_state(arg_name) {
                        None => {
                            return Err(Diagnostic::new(
                                ErrorKind::UndefinedVariable,
                                format!("UndefinedVariable: `{arg_name}`"),
                            ))
                        }
                        Some(crate::rules::ValueState::Consumed) => {
                            return Err(Diagnostic::new(
                                ErrorKind::UseAfterMove,
                                format!(
                                    "UseAfterMove: `{arg_name}` was already consumed"
                                ),
                            ))
                        }
                        Some(crate::rules::ValueState::Uninitialized) => {
                            return Err(Diagnostic::marked(
                                ErrorKind::CompileError,
                                "UseBeforeInit",
                                format!(
                                    "UseBeforeInit: var `{arg_name}` is read before assignment"
                                ),
                            ))
                        }
                        _ => {}
                    }
                    let arg_name = arg_name.clone();
                    self.consume_binding(&arg_name, "extern move argument")?;
                }
            }
        }
        if ext.fallible {
            return self.extern_call_return(name, ext);
        }
        Ok(extern_ret_type(&ext.ret_abi))
    }
}
