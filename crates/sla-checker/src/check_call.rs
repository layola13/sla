//! Call checking, mirroring the `call_expr` dispatch order of
//! `type_checker.zig` (5050-5140, ~4341): 1. `sum` specials;
//! 2. `extern_funcs` FFI bypass (types ignored; arity/`^`/ABI only);
//! 3. builtins `stack_alloc`/`panic`/`panic_msg`; 4. user macros
//! (consumption via rules::macro_param_consumption); 5. user /
//! imported functions (R-6 discipline, check_call_consume.rs);
//! 6. struct constructors and fn-ptr locals. Associated-target
//! consumption specials (`mem::forget`, `Box::into_raw`,
//! `ManuallyDrop::into_inner`) precede generic associated resolution.

use crate::checker::{ChainStages, Checker, FuncSig};
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
            let (params, ret, is_variadic) = match &callee_ty {
                Type::Closure(c) => (c.params.clone(), (*c.ret).clone(), false),
                Type::FnPtr(f) => (f.params.clone(), (*f.ret).clone(), f.is_variadic),
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
            // Extern variadic batch (LINE-CENSUS-VARIADIC): a
            // variadic fn-pointer callee accepts any argument
            // count at or above its fixed prefix; trailing
            // arguments are checked as expressions only, exactly
            // as on the named-variadic paths (check_extern /
            // check_overload2 match over the fixed prefix only).
            if call.args.len() < params.len()
                || (!is_variadic && call.args.len() != params.len())
            {
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
            for arg in call.args.iter().skip(params.len()) {
                self.check_expr(arg)?;
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
        // DEF-RUST-FITER-SUMARM-SHADOW: this arm had no `funcs`
        // gate at all, so a user free `fn sum` (or extern `fn
        // sum`) was unreachable — a non-collection argument was
        // rejected as `sum over {Type}` and a collection
        // argument was accepted as the builtin while the
        // codegen emitted the user fn (checker/codegen
        // disagreement, P1). The gate below is the same shape
        // as the count/max gate, extended to every function
        // registry (see `user_fn_owns_name`): when a user
        // function owns `sum` and the argument is not a direct
        // iterator chain, the call falls through to the plain
        // function stages below. A direct chain argument is
        // the builtin window terminal in method form
        // (`v.iter().sum()`), which the gate must not
        // pre-empt — the checker and the codegen agree the
        // terminal owns chain arguments (see
        // `call_arg_is_iter_chain`).
        if name == "sum" {
            // Zig's order: receiver-inferred resolution
            // (type_checker.zig :9669) runs long before the
            // bare-name `sum` arm (:10693), so a user type's own
            // `sum` method (inherent or trait) wins over the
            // builtin element sum. This port's phase-1 order put
            // the special first, so `triple.sum()` on a struct
            // with an inherent `sum` was rejected as "sum over
            // {Type}" without ever trying the method
            // (test_unit_impl_static_methods,
            // test_unit_trait_static_dispatch). No std receiver
            // leaf knows `sum`, so consulting the receiver
            // cascade here can only resolve user methods; the
            // builtin arm below keeps its verdicts otherwise
            // (sum over Option still TypeMismatch).
            if let Some(result) = self.check_receiver_call(call) {
                return result;
            }
            if self.call_arg_is_iter_chain(call) || !self.user_fn_owns_name(name) {
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
            // A user function owns `sum` and the argument is
            // not a chain: fall through to the plain function
            // stages below.
        }

        // 1b. `count` / `max` specials (LINE-FITER-TEXT-PORT):
        // the iterator-window terminals whose Zig arms sit in
        // the same family as `sum` (type_checker.zig :10824
        // count -> u64, :10848 max -> Option<elem> with the
        // numeric element gate). Same order discipline as the
        // `sum` arm above: receiver-inferred resolution runs
        // first, so a user type's own `count` / `max` method
        // wins over the builtin window terminal; the builtin
        // arm keeps its verdicts otherwise. Before this arm,
        // `count` resolved nowhere (UndefinedVariable at check
        // time on both routes, LINE-BEND2-TRIAGE minim2) and
        // `max` likewise.
        //
        // The funcs gate is load-bearing: a user-DECLARED
        // function named `count` / `max` (e.g. the R-3 suite's
        // `fn count(x: &Vec<String>) -> i32`, fixture_03's
        // two-argument `fn max(a, b)`) is resolved by the plain
        // function stages below and must never be pre-empted —
        // the window arm claims a call only when no declared
        // function owns the name (the overload-member stage at
        // 1a uses the same `funcs.contains_key` probe).
        // DEF-RUST-FITER-SUMARM-SHADOW: the gate checked
        // `funcs` ONLY, so an extern `fn max` / `fn count`
        // (`.sai` contract, in `extern_funcs`) was claimed by
        // the builtin arm before the extern FFI bypass ran.
        // The gate now covers every function registry (see
        // `user_fn_owns_name`), and — as for `sum` above — a
        // direct iterator-chain argument is the builtin
        // terminal in method form, which the gate must not
        // pre-empt: without the chain guard a user fn named
        // `count` / `max` misresolved `v.iter().count()` /
        // `v.iter().max()` to the user fn at check time while
        // the codegen emitted the terminal (a bool-returning
        // user `fn count` failed the method form at check
        // time; an i64-returning user `fn max` made the
        // method call's Option result unmatchable). The
        // receiver cascade now runs before the gate (as in the
        // `sum` arm), so a user type's own `count` / `max`
        // method wins even when a free fn of the same name
        // exists.
        if name == "count" || name == "max" {
            if let Some(result) = self.check_receiver_call(call) {
                return result;
            }
            if self.call_arg_is_iter_chain(call) || !self.user_fn_owns_name(name) {
                if call.args.len() != 1 {
                    return Err(Diagnostic::new(
                        ErrorKind::InvalidArgsCount,
                        format!("{name} expects exactly 1 argument"),
                    ));
                }
                let target_ty = self.check_expr(&call.args[0])?;
                return if name == "count" {
                    crate::check_assoc_helpers::check_count_call(self, &target_ty)
                } else {
                    crate::check_assoc_helpers::check_max_call(self, &target_ty)
                };
            }
            // A user function owns the name and the argument is
            // not a chain: fall through to the plain function
            // stages below.
        }

        // 1a-member. Direct call to one overload member by its
        // member symbol (`pick__ov_f64(2.5)`): the symbol names a
        // set member, not a declared function, so no other stage
        // can resolve it. Resolve against the member signature
        // and record the member symbol so the text emitter binds
        // the call to the mangled member definition. A local
        // binding or a plain function of the same spelling keeps
        // priority (falls through to the usual stages).
        if self.lookup_ty(name).is_none() && !self.funcs.contains_key(name) {
            if let Some((base, sig)) = self.overload_member_by_symbol(name) {
                let tys: Vec<Type> =
                    sig.params.iter().map(|p| p.ty.clone()).collect();
                self.overload_call_symbols.insert(
                    crate::types_util::node_key_of(node),
                    sla_ast::overload_member_symbol(&base, &tys),
                );
                return self.check_call_args_against_sig(
                    name,
                    &sig,
                    &call.args,
                    &call.generics,
                );
            }
        }

        // 1b. Overload unification (check_overload2.rs): when a
        // set exists for this name in ANY source (func / imported /
        // extern contract), candidates from every source compete
        // under one resolution. This must precede the single-
        // signature FFI bypass below: a promoted extern name no
        // longer lives in `extern_funcs`, and a singleton extern
        // must still join the pool when another source has a set.
        if let Some(result) = self.try_resolve_unified_call(
            name,
            call,
            crate::types_util::node_key_of(node),
        ) {
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
                    Some((c.params.clone(), (*c.ret).clone(), false))
                }
                Type::FnPtr(f) => {
                    Some((f.params.clone(), (*f.ret).clone(), f.is_variadic))
                }
                _ => None,
            };
            if let Some((params, ret, is_variadic)) = callable {
                // Extern variadic batch (LINE-CENSUS-VARIADIC):
                // a variadic fn-pointer binding accepts any
                // argument count at or above its fixed prefix;
                // trailing arguments are checked as expressions
                // only (the named-variadic rule).
                if call.args.len() < params.len()
                    || (!is_variadic && call.args.len() != params.len())
                {
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
                for arg in call.args.iter().skip(params.len()) {
                    self.check_expr(arg)?;
                }
                return Ok(ret);
            }
        }

        // 5. User functions / imported signatures / overload sets.
        if let Some(result) = self.resolve_user_call(name, call, crate::types_util::node_key_of(node)) {
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
            // Extern variadic batch (LINE-CENSUS-VARIADIC): the
            // fixed prefix is a floor for a variadic fn-pointer.
            if call.args.len() < f.params.len()
                || (!f.is_variadic && f.params.len() != call.args.len())
            {
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
            for arg in call.args.iter().skip(f.params.len()) {
                self.check_expr(arg)?;
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

    /// DEF-RUST-FITER-SUMARM-SHADOW: whether a user function
    /// owns this name, in ANY function registry. The builtin
    /// sum/count/max terminals must yield to all of them:
    /// locally-declared singletons (`funcs`), locally-declared
    /// overload sets (`func_overload_sets` — a name promoted
    /// to a set is REMOVED from `funcs`, check_program.rs,
    /// so checking `funcs` alone misses overloaded user
    /// functions), extern contracts (`extern_funcs` /
    /// `extern_overload_sets`, same promotion rule), and
    /// imported signatures (`imported_funcs` /
    /// `imported_overload_sets`, registered from `.sa`
    /// imports by expand_wire.rs).
    fn user_fn_owns_name(&self, name: &str) -> bool {
        self.funcs.contains_key(name)
            || self.func_overload_sets.contains_key(name)
            || self.extern_funcs.contains_key(name)
            || self.extern_overload_sets.contains_key(name)
            || self.imported_funcs.contains_key(name)
            || self.imported_overload_sets.contains_key(name)
    }

    /// DEF-RUST-FITER-HELDCHAIN-RESIDUAL: reduce an iterator-chain
    /// expression (direct or held) to its stages, mirroring the
    /// codegen's `iter_chain_plan_ref` + `iter_chain_plan_held`
    /// (iter_window.rs) exactly — including the duplicate skip/take
    /// rejection and the held-composition refusal (an outer skip
    /// over a held skip, or take over take, is not a single
    /// window). Returns None when the expression is not a chain.
    /// The direct walk and the held walk are one loop here (the
    /// codegen splits them into ref/held because the ref form
    /// borrows original nodes while the held form clones stored
    /// plans; the checker only needs the stages, so one walk
    /// serves both): `iter` / `into_iter` closes a direct chain,
    /// an identifier closes a held chain through the
    /// chain-binding table (see `chain_stages_for_let`).
    pub(crate) fn iter_chain_stages(&self, node: &Node) -> Option<ChainStages> {
        let mut skip_seen = false;
        let mut take_seen = false;
        let mut cur: &Node = node;
        loop {
            match cur {
                Node::CallExpr(c) => {
                    if (c.func_name == "iter" || c.func_name == "into_iter")
                        && c.args.len() == 1
                    {
                        return Some(ChainStages {
                            skip: skip_seen,
                            take: take_seen,
                        });
                    }
                    if c.func_name == "rev" && c.args.len() == 1 {
                        cur = &c.args[0];
                        continue;
                    }
                    if c.func_name == "skip" && c.args.len() == 2 {
                        if skip_seen {
                            return None;
                        }
                        skip_seen = true;
                        cur = &c.args[0];
                        continue;
                    }
                    if c.func_name == "take" && c.args.len() == 2 {
                        if take_seen {
                            return None;
                        }
                        take_seen = true;
                        cur = &c.args[0];
                        continue;
                    }
                    if c.func_name == "copied" && c.args.len() == 1 {
                        cur = &c.args[0];
                        continue;
                    }
                    return None;
                }
                Node::Identifier(name) => {
                    // Held-chain binding: look up the innermost
                    // binding and its recorded stages. Refuse
                    // exactly as the codegen does
                    // (`iter_chain_plan_held`): an outer skip over
                    // a held skip (or take over take) is not a
                    // single window, so the call is not a chain
                    // and resolves to the user fn at both levels.
                    // A name that is not a scope binding (a
                    // function, a type) is not a chain either.
                    let binding = self.scopes.lookup(name)?;
                    let stages = self.chain_bindings.get(&binding.id)?;
                    if skip_seen && stages.skip {
                        return None;
                    }
                    if take_seen && stages.take {
                        return None;
                    }
                    return Some(ChainStages {
                        skip: skip_seen || stages.skip,
                        take: take_seen || stages.take,
                    });
                }
                _ => return None,
            }
        }
    }

    /// DEF-RUST-FITER-HELDCHAIN-RESIDUAL: the chain stages a let
    /// site records for its binding (mirrors codegen
    /// `iter_chain_plan_for_let` + gen_let_stmt, which records
    /// only when the value is a CallExpr). A non-call value —
    /// including a bare identifier alias (`let d = c;`) — records
    /// nothing, exactly as in the codegen: the alias is not a
    /// chain binding there, so `d.sum()` is not a terminal there
    /// either. Callers compute the stages BEFORE defining the
    /// binding (see `check_let`): the value's identifiers must
    /// resolve to the pre-let bindings — a shadowing let's value
    /// refers to the outer binding, mirroring the codegen, which
    /// resolves the RHS before pushing the new alias.
    pub(crate) fn chain_stages_for_let(&self, value: &Node) -> Option<ChainStages> {
        if !matches!(value, Node::CallExpr(_)) {
            return None;
        }
        self.iter_chain_stages(value)
    }

    /// Whether the call's single argument is an iterator chain
    /// (direct or held — see `iter_chain_stages`). Mirrors the
    /// codegen's `iter_chain_plan_ref` walk (iter_window.rs) for
    /// the direct form, extended to held-chain bindings: a chain
    /// argument means this call is the builtin window terminal
    /// in method form, which the user-fn gate must not pre-empt
    /// (the checker and the codegen agree the terminal owns chain
    /// arguments, while user functions own non-chain arguments).
    /// Held-chain bindings (an identifier holding a chain) are
    /// detected through the chain-binding table now; the
    /// DEF-RUST-FITER-HELDCHAIN-RESIDUAL fix.
    fn call_arg_is_iter_chain(&self, call: &CallExpr) -> bool {
        if call.args.len() != 1 {
            return false;
        }
        self.iter_chain_stages(&call.args[0]).is_some()
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
