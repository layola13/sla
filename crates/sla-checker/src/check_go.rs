//! R-3 goroutine batch gates: `go` spawn shape, `defer` shape
//! and `select` arms.
//!
//! The diagnostic corpus (`tests/diagnostics/go_diag_*.sla`) is
//! judged by standalone `sla check` rejection (sala doc 13);
//! the rules come from the corpus comments (async interop R1/R2)
//! and the file shapes:
//!
//! * `go f(x)` — the callee must be a NAMED plain function:
//!   a closure value is GoClosureCallee, a method call is
//!   GoMethodCallee, an `async fn` is GoAsyncCallee (R1: async
//!   functions are driven through block_on, never spawned), a
//!   non-void return is GoNonVoidCallee, and more than one
//!   argument is GoArity.
//! * spawn arguments may not smuggle borrows across the
//!   goroutine boundary: an argument whose type mentions a
//!   borrow (directly, or inside a struct with borrow fields —
//!   reached through pointers too) is GoBorrowPayload.
//! * the spawned entry receives exactly one word, so a spawned
//!   function's single parameter must be a pointer or word-sized
//!   scalar (GoNonWordParam): primitives, pointers, fn-pointers
//!   and `Chan` pass; structs (even word-sized ones), tuples,
//!   arrays, `String`, `Vec`, borrows and futures do not —
//!   mirroring Zig `goParamIsWordOrPtr` (type_checker.zig:4324).
//!   Box larger values and pass the pointer instead.
//! * arena route 6: inside an `arena {}` domain, spawning with
//!   an arena-owned argument is ArenaGoroutine (the arena
//!   checker's synthetic-`go` route, driven here with the real
//!   call so its argument walk sees the spawn's arguments).
//! * `defer` requires a call operand (DeferNonCall otherwise);
//!   the call itself is checked normally.
//! * a BLOCKING `select` may not appear in an `async fn`
//!   (GoAsyncSelect, R2's async-select diag): no default arm and
//!   no timeout arm, mirroring Zig `checkSelectStmt`; a select
//!   with a default or timeout arm is non-blocking and stays
//!   legal. Case arms check their receive call and bind the
//!   pattern names with the payload type.

use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::stmts::{DeferStmt, GoStmt, SelectStmt};
use sla_ast::types::Type;

use crate::checker::{Checker, FuncSig, ParamSig};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::ValueState;
use crate::types_util::u64_ty;

fn err(kind: ErrorKind, msg: String) -> Diagnostic {
    Diagnostic::new(kind, msg)
}

/// Mirror of Zig `goParamIsWordOrPtr` (type_checker.zig:4324):
/// the spawned entry receives exactly one word, so the callee's
/// single parameter must be a primitive (non-void), a pointer, a
/// fn-pointer, or `Chan`. Structs (even word-sized ones), tuples,
/// arrays, `String`, `Vec`, borrows, futures and closures are
/// rejected — box larger values and pass the pointer instead.
/// (`String`/`Vec` are `UserDefined` here, so the name test
/// rejects them exactly as Zig's does.)
fn go_param_is_word_or_ptr(ty: &Type) -> bool {
    match ty {
        Type::Primitive(p) => {
            !matches!(p, sla_ast::types::Primitive::VoidType)
        }
        Type::Pointer(_) => true,
        Type::FnPtr(_) => true,
        Type::UserDefined(u) => u.name == "Chan",
        _ => false,
    }
}

impl Checker {
    /// Does this type carry a borrow anywhere the spawn would
    /// have to move across the boundary? Structs recurse through
    /// their registered fields; pointers do NOT stop the walk
    /// (the corpus passes `*Struct{ r: &u64 }`).
    ///
    /// `pub(crate)`: the `thread::spawn` closure guard
    /// (check_spawn_guard.rs) reuses this exact predicate — the
    /// borrow-only containment test, not the wider
    /// `typeForbiddenAcrossBoundary` (closures/futures are legal
    /// cargo for the async thread pool).
    pub(crate) fn go_type_has_borrow(&self, ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        match ty {
            Type::Borrow(_) => true,
            Type::Pointer(inner) => self.go_type_has_borrow(inner, depth + 1),
            Type::Array(a) => self.go_type_has_borrow(&a.elem, depth + 1),
            Type::Tuple(t) => t
                .elems
                .iter()
                .any(|e| self.go_type_has_borrow(e, depth + 1)),
            Type::UserDefined(u) => {
                if let Some(info) = self.structs.get(&u.name) {
                    let fields: Vec<Type> =
                        info.fields.iter().map(|f| f.1.clone()).collect();
                    return fields
                        .iter()
                        .any(|f| self.go_type_has_borrow(f, depth + 1));
                }
                u.generics
                    .iter()
                    .any(|g| self.go_type_has_borrow(g, depth + 1))
            }
            _ => false,
        }
    }

    pub fn check_go_stmt(&mut self, stmt: &GoStmt) -> CheckResult<Type> {
        let Node::CallExpr(call) = &*stmt.call else {
            return Err(err(
                ErrorKind::GoClosureCallee,
                "GoClosureCallee: `go` requires a call to a named function"
                    .to_string(),
            ));
        };
        // R-3: spawning CONSUMES the payload's handle, not the
        // payload binding (a raw pointer crosses by copy — the
        // goroutine owns the pointee). Suppress the ownership
        // ledger for everything below in this function.
        self.no_consume_depth = self.no_consume_depth.saturating_add(1);
        let shape = self.check_go_shape_inner(stmt, call);
        self.no_consume_depth = self.no_consume_depth.saturating_sub(1);
        shape
    }

    fn check_go_shape_inner(
        &mut self,
        stmt: &GoStmt,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        // Method-form calls parse as CallExpr with an associated
        // target or a callee expression; closure callees carry an
        // explicit callee node.
        if call.callee.is_some() {
            return Err(err(
                ErrorKind::GoClosureCallee,
                "GoClosureCallee: `go` cannot spawn a closure value; spawn a named function".to_string(),
            ));
        }
        if call.associated_target.is_some() {
            return Err(err(
                ErrorKind::GoMethodCallee,
                "GoMethodCallee: `go` cannot spawn a method call; spawn a named function".to_string(),
            ));
        }
        // Method form in this AST is a plain CallExpr whose
        // receiver rides args[0] and whose name is a method of
        // the receiver's type (not a free function).
        if !call.args.is_empty()
            && !self.funcs.contains_key(&call.func_name)
            && !self.imported_funcs.contains_key(&call.func_name)
        {
            if let Ok(recv_ty) = self.check_expr(&call.args[0]) {
                if let Some(tname) =
                    crate::types_util::concrete_type_name(&recv_ty)
                {
                    let key = format!("{tname}_{}", call.func_name);
                    if self.funcs.contains_key(&key)
                        || self.imported_funcs.contains_key(&key)
                    {
                        return Err(err(
                            ErrorKind::GoMethodCallee,
                            "GoMethodCallee: `go` cannot spawn a method call; spawn a named function".to_string(),
                        ));
                    }
                }
            }
        }
        let name = call.func_name.clone();
        // F-G8: a scope binding of fn-pointer type is a legal
        // spawn entry — Zig `checkGoExpr` accepts `.fn_ptr`
        // symbols (this is how sa_std/supervisor.sla's
        // `go supervised entry(arg)` checks: `entry: fn(u64)`
        // is a parameter). Only closure values and
        // non-function bindings keep the GoClosureCallee
        // rejection below.
        let fn_ptr_sig = match self.lookup_ty(&name) {
            Some(Type::FnPtr(fp)) if !self.funcs.contains_key(&name) => {
                Some(FuncSig {
                    params: fp
                        .params
                        .iter()
                        .map(|ty| ParamSig {
                            name: String::new(),
                            ty: ty.clone(),
                            is_borrow: false,
                            is_move: false,
                            is_var: false,
                        })
                        .collect(),
                    ret_ty: (*fp.ret).clone(),
                    is_pub: true,
                    ret_fallible_sugar: false,
                    ret_borrow_origin: None,
                    is_async: false,
                    is_variadic: false,
                    param_may_write: Vec::new(),
                })
            }
            _ => None,
        };
        if fn_ptr_sig.is_none()
            && self.lookup_ty(&name).is_some()
            && !self.funcs.contains_key(&name)
        {
            return Err(err(
                ErrorKind::GoClosureCallee,
                format!(
                    "GoClosureCallee: `go` cannot spawn the closure value `{name}`; spawn a named function"
                ),
            ));
        }
        let sig = self
            .funcs
            .get(&name)
            .cloned()
            .or_else(|| self.imported_funcs.get(&name).cloned())
            .or(fn_ptr_sig);
        let Some(sig) = sig else {
            // Unknown name: let the ordinary call check name the
            // fault (UndefinedCall etc.).
            return self.check_expr(&stmt.call);
        };
        // Arena route 6 precedes the shape gates: crossing the
        // goroutine boundary with an arena-owned value is the
        // arena fault regardless of the callee's own shape.
        let synth = CallExpr {
            func_name: "go".to_string(),
            associated_target: None,
            generics: Vec::new(),
            args: call.args.clone(),
            callee: None,
        };
        self.arena_check_call(&synth, None)?;
        if sig.is_async {
            return Err(err(
                ErrorKind::GoAsyncCallee,
                format!(
                    "GoAsyncCallee: `go` cannot spawn async fn `{name}`; drive it with block_on inside a synchronous goroutine"
                ),
            ));
        }
        if !matches!(sig.ret_ty, Type::Primitive(p) if p == sla_ast::types::Primitive::VoidType) {
            return Err(err(
                ErrorKind::GoNonVoidCallee,
                format!(
                    "GoNonVoidCallee: spawned function `{name}` must return void"
                ),
            ));
        }
        if call.args.len() > 1 {
            return Err(err(
                ErrorKind::GoArity,
                format!(
                    "GoArity: `go` spawns take at most one argument, got {}",
                    call.args.len()
                ),
            ));
        }
        // (A) DEF-RUST-GO-STRUCTARG-PREMATURE-RELEASE: the spawned
        // entry receives exactly one word — mirror Zig
        // `goParamIsWordOrPtr` and reject a callee whose single
        // parameter is not a pointer or word-sized scalar. Only
        // when the call actually supplies the one argument, so a
        // bare arity slip keeps its own diagnostic.
        if call.args.len() == 1
            && sig.params.len() == 1
            && !go_param_is_word_or_ptr(&sig.params[0].ty)
        {
            return Err(err(
                ErrorKind::GoNonWordParam,
                format!(
                    "GoNonWordParam: go: parameter of spawned function `{name}` must be a pointer or word-sized scalar (the entry receives one word); box larger values and pass the pointer"
                ),
            ));
        }
        // Argument types first (ordinary discipline), then the
        // borrow-payload walk over the checked types.
        let mut arg_tys = Vec::new();
        for arg in &call.args {
            arg_tys.push(self.check_expr(arg)?);
        }
        for ty in &arg_tys {
            if self.go_type_has_borrow(ty, 0) {
                return Err(err(
                    ErrorKind::GoBorrowPayload,
                    "GoBorrowPayload: spawn argument carries a borrow across the goroutine boundary".to_string(),
                ));
            }
        }
        // F-G7: Zig `checkGoExpr` types a `go` expression as
        // u64 (the raw task handle loaded from the spawn slot),
        // not JoinHandle<void> -- JoinHandle<T> belongs to the
        // thread::spawn surface only. Typing it JoinHandle made
        // the handle non-Copy, so call-arg planning emitted a
        // `^h` move into sa_task_join(handle: u64) and every
        // u64 consumer (struct fields, (u64, i32) tuples,
        // monitor(handle)) mismatched at check time.
        Ok(u64_ty())
    }

    pub fn check_defer_stmt(&mut self, stmt: &DeferStmt) -> CheckResult<()> {
        if !matches!(&*stmt.call, Node::CallExpr(_)) {
            return Err(err(
                ErrorKind::DeferNonCall,
                "DeferNonCall: `defer` requires a call expression".to_string(),
            ));
        }
        // The defer desugarer clones the callee shape into the
        // exit path; an immediately-invoked closure literal has
        // no cloneable callee shape (corpus neg_desugar_failed,
        // expect: DesugarFailed).
        if let Node::CallExpr(call) = &*stmt.call {
            if call.callee.is_some() {
                return Err(err(
                    ErrorKind::DesugarFailed,
                    "DesugarFailed: `defer` of an immediately-invoked closure cannot be desugared (callee shape cannot be cloned)".to_string(),
                ));
            }
        }
        self.check_expr(&stmt.call)?;
        Ok(())
    }

    pub fn check_select_stmt(
        &mut self,
        stmt: &SelectStmt,
        stmt_key: usize,
    ) -> CheckResult<()> {
        // Zig `checkSelectStmt`: inside an async fn only a blocking
        // select is forbidden (it would park the executor); a select
        // carrying a default arm or a timeout arm never blocks and
        // stays legal (corpus test_unit_go_async_interop).
        let is_async = self
            .current_func
            .as_ref()
            .map(|f| f.is_async)
            .unwrap_or(false);
        let non_blocking =
            stmt.timeout.is_some() || stmt.arms.iter().any(|arm| arm.is_default);
        if is_async && !non_blocking {
            return Err(err(
                ErrorKind::GoAsyncSelect,
                "GoAsyncSelect: blocking `select` (no default or timeout case) is not available inside an async fn".to_string(),
            ));
        }
        let mut arm_exit: Vec<Vec<String>> = Vec::new();
        for arm in &stmt.arms {
            self.scopes.push_frame();
            let mut arm_result = Ok(());
            if let Some(recv) = &arm.recv {
                match self.check_expr(recv) {
                    Ok(recv_ty) => {
                        // A Chan recv yields the (payload, closed)
                        // pair; the pattern binds its components.
                        let payload = match &recv_ty {
                            Type::Tuple(t) if t.elems.len() == 2 => {
                                t.elems[0].clone()
                            }
                            other => other.clone(),
                        };
                        if let Some((v, closed)) = &arm.pattern {
                            let _ = self.define_symbol_with_state(
                                v,
                                payload.clone(),
                                false,
                                ValueState::Active,
                                None,
                            );
                            if let Some(closed) = closed {
                                let _ = self.define_symbol_with_state(
                                    closed,
                                    crate::types_util::bool_ty(),
                                    false,
                                    ValueState::Active,
                                    None,
                                );
                            }
                        }
                    }
                    Err(e) => arm_result = Err(e),
                }
            }
            if arm_result.is_ok() {
                arm_result = self.check_block(&arm.body);
            }
            let arm_frame = self.scopes.pop_frame();
            arm_exit.push(crate::check_program::block_exit_cleanups(
                &arm_frame.bindings,
            ));
            arm_result?;
        }
        if let Some(timeout) = &stmt.timeout {
            self.check_expr(&timeout.expr)?;
            self.scopes.push_frame();
            let body_result = self.check_block(&timeout.body);
            let timeout_frame = self.scopes.pop_frame();
            arm_exit.push(crate::check_program::block_exit_cleanups(
                &timeout_frame.bindings,
            ));
            body_result?;
        } else {
            arm_exit.push(Vec::new());
        }
        // The arm pattern bindings live in the arm frame, not in
        // any statement list, so no statement cleanup retires
        // them; record the per-arm exit lists (match-arm plan
        // shape) for the emitter, indexed by arm position with
        // the timeout arm at `stmt.arms.len()`.
        self.select_arm_exit_cleanups.insert(stmt_key, arm_exit);
        Ok(())
    }
}
