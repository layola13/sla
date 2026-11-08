//! Extern contract call checking split out of
//! check_overload2.rs for the 700-line file cap (bodies
//! unchanged).

use crate::checker::{
    extern_ret_type, Checker, ExternParam, ExternSig, FuncSig,
};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{is_integer_type, type_desc};
use sla_ast::decls::FuncDecl;
use sla_ast::exprs::{CallExpr, Literal};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

impl Checker {
    /// Zig `checkExternContractCall` (staging :3387), the winning
    /// extern member's real validation: arity (exact, or the fixed
    /// prefix for variadic members), per-parameter move contracts
    /// (a `^` parameter consumes a plain identifier argument;
    /// a move expression already transferred), the closure-to-`ptr`
    /// rejection, and the return type from the ABI spelling — with
    /// the R-6 fallible gate applied on top.
    pub fn check_extern_contract_call(
        &mut self,
        name: &str,
        ext: &ExternSig,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        if ext.params.is_empty() {
            return self.check_extern_headerless_call(name, ext, call);
        }
        if ext.is_variadic {
            if call.args.len() < ext.params.len() {
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
        } else if ext.params.len() != call.args.len() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidArgsCount,
                format!(
                    "extern `{name}` expects {} args, got {}",
                    ext.params.len(),
                    call.args.len()
                ),
            ));
        }
        for (param, arg) in ext.params.iter().zip(call.args.iter()) {
            let arg_ty = self.check_expr(arg)?;
            // A closure has no callable trampoline address; passing
            // one to a by-value `ptr` parameter is a loud error.
            if !param.is_borrow
                && !param.is_move
                && param.ty.trim() == "ptr"
                && (matches!(arg_ty, Type::Closure(_))
                    || matches!(
                        (&arg_ty, arg),
                        (Type::FnPtr(_), Node::MoveExpr(_))
                    ) && matches!(
                        match arg {
                            Node::MoveExpr(m) => &*m.expr,
                            other => other,
                        },
                        Node::ClosureLiteral(_)
                    ))
            {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!("cannot pass a closure to extern parameter `{}` of `{name}`: a closure has no callable trampoline address (its invocation needs the capture pointer); pass a top-level named function instead", param.name),
                ));
            }
            if param.is_move && !matches!(arg, Node::MoveExpr(_)) {
                if let Node::Identifier(arg_name) = arg {
                    let state = self.lookup_state(arg_name);
                    match state {
                        None => {
                            return Err(Diagnostic::new(
                                ErrorKind::UndefinedVariable,
                                format!("UndefinedVariable: `{arg_name}`"),
                            ))
                        }
                        Some(crate::rules::ValueState::Consumed) => {
                            return Err(Diagnostic::new(
                                ErrorKind::UseAfterMove,
                                format!("UseAfterMove: `{arg_name}` was already consumed"),
                            ))
                        }
                        Some(crate::rules::ValueState::Uninitialized) => {
                            return Err(Diagnostic::marked(
                                ErrorKind::CompileError,
                                "UseBeforeInit",
                                format!("UseBeforeInit: var `{arg_name}` is read before assignment"),
                            ))
                        }
                        _ => {}
                    }
                    let arg_name = arg_name.clone();
                    self.consume_binding(&arg_name, "extern move argument")?;
                }
            }
        }
        for arg in call.args.iter().skip(ext.params.len()) {
            self.check_variadic_trailing_arg(name, arg)?;
        }
        if ext.fallible {
            return self.extern_call_return(name, ext);
        }
        Ok(extern_ret_type(&ext.ret_abi))
    }

    /// Zig `checkVariadicTrailingArg` (staging :4031): trailing
    /// variadic arguments must be plain scalar or raw-pointer
    /// values; borrows and moves may not pass through `...`.
    pub fn check_variadic_trailing_arg(
        &mut self,
        call_name: &str,
        arg: &Node,
    ) -> CheckResult<()> {
        if matches!(arg, Node::MoveExpr(_) | Node::BorrowExpr(_)) {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!("variadic argument to {call_name} must be a plain scalar or raw pointer value; borrows and moves may not pass through `...`"),
            ));
        }
        let arg_ty = self.check_expr(arg)?;
        let ok = matches!(
            arg_ty,
            Type::Primitive(
                Primitive::I8
                    | Primitive::I16
                    | Primitive::I32
                    | Primitive::I64
                    | Primitive::U8
                    | Primitive::U16
                    | Primitive::U32
                    | Primitive::U64
                    | Primitive::F32
                    | Primitive::F64
                    | Primitive::Boolean
                    | Primitive::Integer
                    | Primitive::Float
                    | Primitive::RawPtr
            ) | Type::Pointer(_)
        );
        if !ok {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "variadic argument to {call_name} has type {}, which may not pass through `...`: only scalars and raw pointers are allowed (aggregates, String/Vec handles, borrows and owned values are rejected; pass a raw *u8 for C strings)",
                    type_desc(&arg_ty)
                ),
            ));
        }
        Ok(())
    }
}
