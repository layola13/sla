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
        if let Some(callee) = &call.callee {
            // Immediately-invoked closure: the callee must type as a
            // closure; arity and argument types are checked against
            // its parameter list (Zig call arm, callee path).
            let callee_ty = self.check_expr(callee)?;
            let (params, ret) = match &callee_ty {
                Type::Closure(c) => (c.params.clone(), (*c.ret).clone()),
                Type::FnPtr(f) => (f.params.clone(), (*f.ret).clone()),
                other => {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: callee is not callable, actual {}",
                            type_desc(other)
                        ),
                    ))
                }
            };
            if call.args.len() != params.len() {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidArgsCount,
                    format!(
                        "closure expects {} arguments, got {}",
                        params.len(),
                        call.args.len()
                    ),
                ));
            }
            for (arg, expected) in call.args.iter().zip(params.iter()) {
                let arg_ty = self.check_expr(arg)?;
                if !types_equal(expected, &arg_ty) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: closure argument expects {}, actual {}",
                            type_desc(expected),
                            type_desc(&arg_ty)
                        ),
                    ));
                }
            }
            return Ok(ret);
        }
        if let Some(target) = &call.associated_target {
            // Associated forms that resolve (or hard-fail, as in
            // Zig) return here; a miss FALLS THROUGH to the plain
            // resolution stages, exactly as Zig's associated block
            // falls through to them (check_method.rs).
            if let Some(result) = self.check_associated_call(target, call) {
                return result;
            }
        }
        // Name-special arms (INT2-1, check_builtin.rs): in Zig
        // the bare-name block (venue :8805-8895) sits immediately
        // after the associated block and BEFORE closure/fn-ptr
        // locals, user functions, externs and macros, so these
        // names cannot be shadowed. (This crate's `sum` / extern
        // / macro stages below keep their phase-1 relative order;
        // the arms precede all of them, as in Zig.)
        if let Some(result) = self.check_name_special_call(call) {
            return result;
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
            // Zig sum arm: array element, else sliceElementType,
            // else vecElementType — NOT "any generic user type's
            // first parameter" (the phase-1 slice accepted e.g.
            // Option<i32>, which Zig rejects).
            return match target_ty {
                Type::Array(a) => Ok((*a.elem).clone()),
                ref other => {
                    match crate::types_util::slice_element_type(other)
                        .or_else(|| crate::types_util::vec_element_type(other))
                    {
                        Some(elem) => Ok(elem),
                        None => Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            format!("sum over {}", type_desc(other)),
                        )),
                    }
                }
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

        // 3b. Imported macros (registered from .sa imports).
        if let Some(result) = self.check_imported_macro_call(name, call) {
            return result;
        }

        // 4. User macros.
        if let Some(mac) = self.macros.get(name).cloned() {
            for arg in &call.args {
                self.check_expr(arg)?;
            }
            // macro_call_try_cleanups (Zig :5095): when the macro
            // can `?`-propagate, the caller-side cleanup is every
            // active non-internal binding EXCEPT the roots of
            // arguments passed into try-source parameters. Recorded
            // against the call node even when the list is empty
            // (Zig puts unconditionally inside `if (has_macro_try)`).
            if self.macro_has_try(&mac) {
                let mut propagated: Vec<String> = Vec::new();
                for index in 0..mac.params.len() {
                    if self.macro_param_is_try_source(&mac, index) {
                        if let Some(arg) = call.args.get(index) {
                            if let Some(root) =
                                crate::types_util::root_identifier_deep(arg)
                            {
                                propagated.push(root.to_string());
                            }
                        }
                    }
                }
                let cleanup = self.collect_active_names(|b| {
                    propagated.iter().any(|n| n == &b.name)
                });
                self.macro_call_try_cleanups
                    .insert(crate::types_util::node_key_of(node), cleanup);
            }
            for (index, param) in mac.params.iter().enumerate() {
                let consumption =
                    self.macro_param_consumption_recursive(&mac, index);
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
                        // Zig consumes through `rootIdentifier(arg)`
                        // (identifier/index/field chains), not only
                        // bare identifier arguments.
                        let arg_root = call
                            .args
                            .get(index)
                            .and_then(crate::types_util::root_identifier_deep);
                        if let Some(arg_name) = arg_root {
                            let arg_name = arg_name.to_string();
                            match self.lookup_state(&arg_name) {
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
                            self.consume_binding(&arg_name, "user macro move")?;
                        }
                    }
                }
            }
            return Ok(void_ty());
        }

        // 4b. Local closure / fn-pointer variables shadow function
        // names (Zig resolves the callee binding before user
        // functions, ~:4305): a call through a binding of closure
        // or fn-ptr type is checked against that type.
        if let Some(binding_ty) = self.lookup_ty(name) {
            let callable = match &binding_ty {
                Type::Closure(c) => {
                    Some((c.params.clone(), (*c.ret).clone()))
                }
                Type::FnPtr(f) => Some((f.params.clone(), (*f.ret).clone())),
                _ => None,
            };
            if let Some((params, ret)) = callable {
                if call.args.len() != params.len() {
                    return Err(Diagnostic::new(
                        ErrorKind::InvalidArgsCount,
                        format!(
                            "callable `{name}` expects {} arguments, got {}",
                            params.len(),
                            call.args.len()
                        ),
                    ));
                }
                for (arg, expected) in call.args.iter().zip(params.iter()) {
                    let arg_ty = self.check_expr(arg)?;
                    if !types_equal(expected, &arg_ty) {
                        return Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            format!(
                                "TypeMismatch: argument to `{name}` expects {}, actual {}",
                                type_desc(expected),
                                type_desc(&arg_ty)
                            ),
                        ));
                    }
                }
                return Ok(ret);
            }
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

        // Receiver-inferred resolution (inherent -> trait -> dyn
        // -> using extensions), mirroring the venue block at
        // :9669-9725: it runs after plain functions, struct
        // constructors and fn-ptr locals, as in Zig.
        if let Some(result) = self.check_receiver_call(call) {
            return result;
        }

        // Zig's bottom fallback for ANY call that reached the end
        // of the call arm sets "Undefined call: {name}" and returns
        // UndefinedVariable (venue :9982) — plain, dot and
        // associated forms alike (venue probes: `v.double()`
        // without `using`, `b.get()` on the wrong type, and
        // `Box2::nope(b)` all report exactly this).
        Err(Diagnostic::new(
            ErrorKind::UndefinedVariable,
            format!("Undefined call: {name}"),
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

    /// Associated-target calls (`Target::name(...)`), mirroring
    /// the venue associated block (:8438-8810). Returns `None` on
    /// a plain miss so the caller falls through to the remaining
    /// resolution stages, as Zig does; forms that hard-fail in Zig
    /// (trait-target with no impl, arity errors) return
    /// `Some(Err(..))`. Consumption specials first.
    ///
    /// Order inside the block (Zig): trait target -> `ptr` leaves
    /// -> inherent `{Target}_{name}` in funcs ONLY -> the
    /// `Option::None` unit constructor -> trait fallback for the
    /// target type. (The name-keyed static-constructor leaves —
    /// `Vec::new` and kin — are std-surface leaves, not part of
    /// this machinery; see checker.md remaining list.)
    fn check_associated_call(
        &mut self,
        target: &str,
        call: &CallExpr,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        match (target, name) {
            ("mem", "forget") => {
                return Some((|| {
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
                    Ok(void_ty())
                })());
            }
            ("Box", "into_raw") => {
                return Some((|| {
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
                    Ok(raw_ptr_ty())
                })());
            }
            ("ManuallyDrop", "into_inner") => {
                return Some((|| {
                    if call.args.len() != 1 {
                        return Err(Diagnostic::new(
                            ErrorKind::InvalidArgsCount,
                            "ManuallyDrop::into_inner expects 1 argument",
                        ));
                    }
                    let arg_ty = self.check_expr(&call.args[0])?;
                    if let Node::Identifier(arg_name) = &call.args[0] {
                        self.consume_binding(
                            arg_name,
                            "ManuallyDrop::into_inner",
                        )?;
                    }
                    Ok(match arg_ty {
                        Type::UserDefined(u) if !u.generics.is_empty() => {
                            u.generics[0].clone()
                        }
                        other => other,
                    })
                })());
            }
            _ => {}
        }

        // Trait target: `Trait::method(recv, ...)` (venue :8438).
        if self.traits.contains_key(target) {
            return Some(self.check_trait_associated_call(target, call));
        }

        // `ptr` leaves (venue :8457): `null` only; `read_volatile`
        // needs the unsafe-depth counter this crate does not carry
        // and falls through (checker.md remaining list).
        if (target == "ptr" || target == "std__ptr") && name == "null" {
            return Some((|| {
                if !call.args.is_empty() {
                    return Err(Diagnostic::new(
                        ErrorKind::InvalidArgsCount,
                        "ptr::null expects 0 arguments",
                    ));
                }
                if call.generics.len() != 1 {
                    return Err(Diagnostic::new(
                        ErrorKind::InvalidArgsCount,
                        "ptr::null expects 1 generic argument",
                    ));
                }
                Ok(Type::Pointer(Box::new(call.generics[0].clone())))
            })());
        }

        // Inherent: `{Target}_{name}` in funcs ONLY (Zig does not
        // consult imported signatures on this path).
        let method_key = format!("{target}_{name}");
        if let Some(sig) = self.funcs.get(&method_key).cloned() {
            return Some(self.check_method_args(
                name,
                &sig,
                &call.args,
                false,
            ));
        }

        // `Option::None()` unit constructor (venue :8790).
        if target == "Option" && name == "None" {
            return Some((|| {
                if !call.args.is_empty() {
                    return Err(Diagnostic::new(
                        ErrorKind::InvalidArgsCount,
                        "Option::None expects 0 arguments",
                    ));
                }
                Ok(Type::UserDefined(sla_ast::types::UserDefined {
                    name: "Option".to_string(),
                    generics: vec![Type::Infer],
                }))
            })());
        }

        // Trait fallback for the target type (venue :8796): the
        // symbol must live in funcs (an imported-only symbol is an
        // UndefinedVariable in Zig, whose `funcs.get` is the only
        // fetch on this path).
        match self.resolve_trait_method_symbol(
            target,
            None,
            name,
            call.args.len(),
        ) {
            Err(e) => Some(Err(e)),
            Ok(Some(symbol)) => Some(match self.funcs.get(&symbol).cloned() {
                Some(sig) => {
                    self.check_method_args(name, &sig, &call.args, false)
                }
                None => Err(Diagnostic::new(
                    ErrorKind::UndefinedVariable,
                    format!("UndefinedVariable: `{symbol}`"),
                )),
            }),
            Ok(None) => None,
        }
    }

    /// The trait-target associated form (venue :8438):
    /// `Trait::method(recv, ...)` — args[0]'s concrete type must
    /// implement the trait; the declaring trait names the symbol.
    fn check_trait_associated_call(
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
