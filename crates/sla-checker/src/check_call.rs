//! Call checking, mirroring the `call_expr` dispatch order of
//! `type_checker.zig` (5050-5140, ~4341): 1. `sum` specials;
//! 2. `extern_funcs` FFI bypass (types ignored; arity/`^`/ABI only);
//! 3. builtins `stack_alloc`/`panic`/`panic_msg`; 4. user macros
//! (consumption via rules::macro_param_consumption); 5. user /
//! imported functions (R-6 discipline, check_call_consume.rs);
//! 6. struct constructors and fn-ptr locals. Associated-target
//! consumption specials (`mem::forget`, `Box::into_raw`,
//! `ManuallyDrop::into_inner`) precede generic associated resolution.

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
        // R-3: arena call gates (routes 3/5/6); receiver = args[0].
        if crate::check_arena::arena_active() {
            let recv_ty = if !call.args.is_empty() && call.associated_target.is_none() {
                match &call.args[0] {
                    Node::Identifier(recv) => self.lookup_ty(recv),
                    other => self.expr_types.get(&crate::types_util::node_key_of(other)).cloned(),
                }
            } else {
                None
            };
            let is_method = recv_ty.is_some();
            if is_method || call.associated_target.is_none() {
                self.arena_check_call(call, recv_ty.as_ref())?;
            }
        }
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
        // Pre-associated bare-name block (D-C22, check_preassoc.rs):
        // in Zig this segment (venue :8320-8541) sits BEFORE the
        // callee/chan-probe stages' resolution proper and before
        // the associated block — `Some`/`Ok`/`Err`/`ptr__null`
        // fire even with an associated target present.
        if let Some(result) = self.check_pre_assoc_call(node, call) {
            return result;
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
            // Zig sum arm: element-type extraction lives in
            // check_assoc_helpers.rs (R-2 replay split).
            return crate::check_assoc_helpers::check_sum_call(self, &target_ty);
        }

        // 1b. Overload unification (check_overload2.rs): when a
        // set exists for this name in ANY source (func / imported /
        // extern contract), candidates from every source compete
        // under one resolution. This must precede the single-
        // signature FFI bypass below: a promoted extern name no
        // longer lives in `extern_funcs`, and a singleton extern
        // must still join the pool when another source has a set.
        if let Some(result) = self.try_resolve_unified_call(name, call) {
            return result;
        }

        // 2. FFI bypass (single contract, no set anywhere).
        if let Some(ext) = self.extern_funcs.get(name).cloned() {
            return self.check_extern_contract_call(name, &ext, call);
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

        // 3c. `__slice_view(ptr, len)` — the intrinsic the
        // generated sla_std wrappers (gen_wrap.py E_res form) use
        // to build a `&[u8]` view from a raw data pointer + length.
        // Its result is a byte-slice borrow; the borrow-origin
        // annotation on the enclosing function gives it provenance.
        if name == "__slice_view" {
            for arg in &call.args {
                self.check_expr(arg)?;
            }
            // Same shape the parser gives `&[u8]`: Borrow(Slice<u8>).
            return Ok(Type::Borrow(Box::new(
                sla_ast::types::Type::UserDefined(
                    sla_ast::types::UserDefined {
                        name: "Slice".to_string(),
                        generics: vec![crate::types_util::prim(
                            sla_ast::types::Primitive::U8,
                        )],
                    },
                ),
            )));
        }

        // 4. User macros.
        if let Some(mac) = self.macros.get(name).cloned() {
            for arg in &call.args {
                self.check_expr(arg)?;
            }
            // R-3 (J-3/J-4): arity discipline, declared-type
            // argument checks, and the expression-output form.
            // Runs before the consumption ledger so a mis-typed or
            // mis-counted call reports its J-3/J-4 diagnostic.
            let macro_result = self.check_user_macro_typed_call(&mac, call)?;
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
            return Ok(macro_result);
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

        // 5. User functions / imported signatures / overload sets.
        if let Some(result) = self.resolve_user_call(name, call) {
            // R-3: borrow-origin bookkeeping. The non-place gate
            // runs BEFORE the wrapped result is returned: a
            // BorrowOriginNotABinding fault replaces an otherwise
            // successful call verdict (argument-kind rules precede
            // type rules for the annotated parameter). Passing
            // owners to arbitrary calls invalidates their views.
            if result.is_ok() {
                self.origin_call_owner(call)?;
                self.origin_after_call(call);
            }
            return result;
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

        // Receiver-shape std leaf cascade (R-1 port,
        // check_receiver_leaves.rs): the Option/Result/Vec/
        // HashMap/... method leaves and push/pop/remove. In
        // Zig the cascade sits before externs/macros; here it
        // runs after them (this dispatch's standing order,
        // D-C19 family), just before receiver-inferred
        // resolution, which stays the fallback.
        if let Some(result) = self.check_receiver_leaf_call(call) {
            return result;
        }

        // Receiver-inferred resolution (inherent -> trait -> dyn
        // -> using extensions), mirroring the venue block at
        // :9669-9725: it runs after plain functions, struct
        // constructors and fn-ptr locals, as in Zig.
        if let Some(result) = self.check_receiver_call(call) {
            return result;
        }

        // `str::from_utf8` leaf: in Zig it sits after the
        // imported-macro stage (venue :10518), just before the
        // Undefined fallback — the same relative position here.
        if call.associated_target.as_deref() == Some("str")
            && name == "from_utf8"
        {
            return self.check_str_from_utf8_call(call);
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


    /// Associated-target calls (`Target::name(...)`), mirroring
    /// the venue associated block (:8438-8810). Returns `None` on
    /// a plain miss so the caller falls through to the remaining
    /// resolution stages, as Zig does; forms that hard-fail in Zig
    /// (trait-target with no impl, arity errors) return
    /// `Some(Err(..))`. Consumption specials first.
    ///
    /// Order inside the block (Zig): trait target -> `ptr` leaves
    /// -> std-surface associated leaves (`Vec::new` and kin:
    /// check_leaves.rs, R-1 port; then the R-2 constructor
    /// leaves, check_std_ctor.rs, consulted only on an R-1
    /// miss) -> inherent `{Target}_{name}` in funcs ONLY ->
    /// the `Option::None` unit constructor -> trait fallback
    /// for the target type.
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
                    let arg_ty = self.check_expr(&call.args[0])?;
                    if let Node::Identifier(arg_name) = &call.args[0] {
                        self.consume_binding(arg_name, "Box::into_raw")?;
                    }
                    // The raw pointer keeps the box's content
                    // type: `*T` for `Box<T>` (the untyped
                    // raw_ptr_ty() here used to erase payload
                    // shapes the go borrow-payload gate walks).
                    match crate::types_util::box_inner_type(&arg_ty) {
                        Some(inner) => Ok(Type::Pointer(Box::new(inner))),
                        None => Ok(raw_ptr_ty()),
                    }
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

        // `ptr` leaves (venue :8565/:8568): `null`, plus
        // `read_volatile` behind the unsafe gate (the counter
        // D-C21 missed is now carried, checker.rs `unsafe_depth`).
        if (target == "ptr" || target == "std__ptr") && name == "read_volatile" {
            return Some(self.check_read_volatile_call(call));
        }
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

        // Std-surface associated leaves (R-1 port,
        // check_leaves.rs): the Zig associated block's
        // name-keyed static constructors (`Vec::new`, ...),
        // between the `ptr` leaves and the inherent lookup.
        if let Some(result) = self.check_std_assoc_leaf(target, call) {
            return Some(result);
        }

        // Std constructor leaves (R-2 port, check_std_ctor.rs):
        // the same Zig name-keyed constructor set, ported
        // independently by the R-2 line against the pre-R-1
        // base. Consulted only when the R-1 cascade missed, so
        // current-head verdicts win on every shared name; the
        // R-2 cascade additionally carries its DEF-5 shallow-
        // copy push discipline (types_util extractors) used by
        // its receiver leaves.
        if let Some(result) = self.check_std_associated_leaf(target, call) {
            return Some(result);
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

}
