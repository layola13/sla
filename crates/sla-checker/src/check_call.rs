//! Call checking, mirroring the dispatch order of the `call_expr`
//! arm of `type_checker.zig` (arms located at 5050-5140 and ~4341):
//!   1. name-keyed specials (`sum`);
//!   2. `extern_funcs` — the FFI bypass: argument expressions are
//!      checked but their types are IGNORED; only arity, `^` move
//!      contracts, and the ABI return string matter;
//!   3. builtins `stack_alloc` / `panic` / `panic_msg`;
//!   4. user macros — consumption decided by static analysis of the
//!      macro body (rules::macro_param_consumption);
//!   5. user functions and imported signatures — plain by-value
//!      arguments are NOT consumed by the checker (the DEF-5
//!      auto-clone is a codegen behavior, invisible here, D-C7);
//!   6. struct constructors and fn-ptr locals.
//! Associated-target specials with consumption (`mem::forget`,
//! `Box::into_raw`, `ManuallyDrop::into_inner`) sit in the Zig
//! method-call machinery (type_checker.zig:3903/3921/4068) and are
//! handled before generic associated resolution.

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
    pub fn check_call(&mut self, node: &Node, call: &CallExpr) -> CheckResult<Type> {
        let _ = node;
        if call.callee.is_some() {
            return Err(Diagnostic::out_of_slice("immediately-invoked closure call"));
        }
        if let Some(target) = &call.associated_target {
            return self.check_associated_call(target, call);
        }
        let name = call.func_name.as_str();

        // 1. `sum` special: element type of an array / Vec-like.
        if name == "sum" {
            if call.args.len() != 1 {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidArgsCount,
                    "sum expects exactly 1 argument",
                ));
            }
            let target_ty = self.check_expr(&call.args[0])?;
            return match target_ty {
                Type::Array(a) => Ok((*a.elem).clone()),
                Type::UserDefined(u) if !u.generics.is_empty() => Ok(u.generics[0].clone()),
                other => Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!("sum over {}", type_desc(&other)),
                )),
            };
        }

        // 2. FFI bypass.
        if let Some(ext) = self.extern_funcs.get(name).cloned() {
            if ext.move_params.len() != call.args.len() {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidArgsCount,
                    format!(
                        "extern `{name}` expects {} args, got {}",
                        ext.move_params.len(),
                        call.args.len()
                    ),
                ));
            }
            for (is_move, arg) in ext.move_params.iter().zip(call.args.iter()) {
                self.check_expr(arg)?;
                if *is_move {
                    if let Node::Identifier(arg_name) = arg {
                        self.consume_binding(arg_name, "extern move argument")?;
                    }
                }
            }
            return Ok(crate::checker::extern_ret_type(&ext.ret_abi));
        }

        // 3. Builtins.
        if name == "stack_alloc" || crate::checker::is_panic_builtin_name(name) {
            for arg in &call.args {
                self.check_expr(arg)?;
            }
            return Ok(if name == "stack_alloc" {
                raw_ptr_ty()
            } else {
                void_ty()
            });
        }

        // 4. User macros.
        if let Some(mac) = self.macros.get(name).cloned() {
            for arg in &call.args {
                self.check_expr(arg)?;
            }
            for (index, param) in mac.params.iter().enumerate() {
                let consumption =
                    crate::rules::macro_param_consumption(&mac.body, param);
                match consumption {
                    MacroParamConsumption::Never => {}
                    MacroParamConsumption::Conditional => {
                        return Err(Diagnostic::marked(
                            ErrorKind::CompileError,
                            "MacroConditionalConsume",
                            format!(
                                "MacroConditionalConsume: macro `{name}` consumes parameter `{param}` only on some control-flow paths"
                            ),
                        ));
                    }
                    MacroParamConsumption::Always => {
                        if let Some(Node::Identifier(arg_name)) = call.args.get(index) {
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
                                _ => {}
                            }
                            self.consume_binding(arg_name, "user macro move")?;
                        }
                    }
                }
            }
            return Ok(void_ty());
        }

        // 5. User functions / imported signatures.
        let sig = self
            .funcs
            .get(name)
            .cloned()
            .or_else(|| self.imported_funcs.get(name).cloned());
        if let Some(sig) = sig {
            return self.check_call_args_against_sig(name, &sig, &call.args);
        }

        // 6a. Struct constructor call form.
        if let Some(info) = self.structs.get(name).cloned() {
            if info.fields.len() != call.args.len() {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidArgsCount,
                    format!(
                        "struct `{name}` constructor expects {} args, got {}",
                        info.fields.len(),
                        call.args.len()
                    ),
                ));
            }
            for ((_, field_ty), arg) in info.fields.iter().zip(call.args.iter()) {
                let arg_ty = self.check_expr(arg)?;
                if !value_assignable_to(field_ty, &arg_ty) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: constructor arg expects {}, actual {}",
                            type_desc(field_ty),
                            type_desc(&arg_ty)
                        ),
                    ));
                }
            }
            return Ok(Type::UserDefined(sla_ast::types::UserDefined {
                name: name.to_string(),
                generics: Vec::new(),
            }));
        }

        // 6b. Fn-ptr valued local binding.
        if let Some(Type::FnPtr(f)) = self.lookup_ty(name) {
            if f.params.len() != call.args.len() {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidArgsCount,
                    "fn_ptr call arity mismatch",
                ));
            }
            for (param_ty, arg) in f.params.iter().zip(call.args.iter()) {
                let arg_ty = self.check_expr(arg)?;
                if !types_equal(param_ty, &arg_ty) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        "fn_ptr call argument type mismatch",
                    ));
                }
            }
            return Ok((*f.ret).clone());
        }

        // Zig's wrapper synthesizes exactly this message for calls it
        // cannot resolve (checkExpr fallback, type_checker.zig:3143).
        Err(Diagnostic::new(
            ErrorKind::CompileError,
            format!(
                "checkExpr failed at call {name} with {} args",
                call.args.len()
            ),
        ))
    }

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
        for (param, arg) in sig.params.iter().zip(args.iter()) {
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
            // Borrow parameters accept the bare value (auto-borrow) or
            // an explicit borrow of the parameter type.
            let matched = if param.is_borrow {
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
        }
        Ok(sig.ret_ty.clone())
    }

    /// Associated-target calls (`Target::name(...)` / receiver-style
    /// recorded with a target). Consumption specials first, then
    /// qualified-name resolution (`Target__name`, then `Target.name`
    /// style keys, then the plain name).
    fn check_associated_call(
        &mut self,
        target: &str,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        let name = call.func_name.as_str();
        match (target, name) {
            ("mem", "forget") => {
                if call.args.len() != 1 {
                    return Err(Diagnostic::new(
                        ErrorKind::InvalidArgsCount,
                        "mem::forget expects 1 argument",
                    ));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                if let Node::Identifier(arg_name) = &call.args[0] {
                    if !self.type_is_copy(&arg_ty)
                        && !crate::types_util::is_borrow_like_type(&arg_ty)
                    {
                        self.consume_binding(arg_name, "mem::forget")?;
                    }
                }
                return Ok(void_ty());
            }
            ("Box", "into_raw") => {
                if call.args.len() != 1 {
                    return Err(Diagnostic::new(
                        ErrorKind::InvalidArgsCount,
                        "Box::into_raw expects 1 argument",
                    ));
                }
                self.check_expr(&call.args[0])?;
                if let Node::Identifier(arg_name) = &call.args[0] {
                    self.consume_binding(arg_name, "Box::into_raw")?;
                }
                return Ok(raw_ptr_ty());
            }
            ("ManuallyDrop", "into_inner") => {
                if call.args.len() != 1 {
                    return Err(Diagnostic::new(
                        ErrorKind::InvalidArgsCount,
                        "ManuallyDrop::into_inner expects 1 argument",
                    ));
                }
                let arg_ty = self.check_expr(&call.args[0])?;
                if let Node::Identifier(arg_name) = &call.args[0] {
                    self.consume_binding(arg_name, "ManuallyDrop::into_inner")?;
                }
                return Ok(match arg_ty {
                    Type::UserDefined(u) if !u.generics.is_empty() => {
                        u.generics[0].clone()
                    }
                    other => other,
                });
            }
            _ => {}
        }
        let candidates = [
            format!("{target}__{name}"),
            format!("{target}_{name}"),
            name.to_string(),
        ];
        for cand in &candidates {
            let sig = self
                .funcs
                .get(cand)
                .cloned()
                .or_else(|| self.imported_funcs.get(cand).cloned());
            if let Some(sig) = sig {
                return self.check_call_args_against_sig(cand, &sig, &call.args);
            }
        }
        Err(Diagnostic::new(
            ErrorKind::UndefinedVariable,
            format!("UndefinedVariable: `{target}::{name}`"),
        ))
    }
}
