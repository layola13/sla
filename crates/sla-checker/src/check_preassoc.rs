//! Pre-associated bare-name special block, mirroring the
//! segment of the Zig checker's `call_expr` arm that sits BEFORE
//! the associated-target block (venue head 2cc12ea8,
//! type_checker.zig:8320-8541; the D-C22 census segment). Because
//! it runs first, these names pre-empt associated resolution,
//! user functions, externs and macros alike — venue probes: a
//! user `fn Some(a, b)` cannot make `Some(1, 2)` legal, and
//! `foo::Some(42)` / `foo::Ok(42)` still construct Option/Result
//! (the `Some`/`Ok`/`Err`/`ptr__null` predicates ignore the
//! associated target entirely; only `None` and `monitor` require
//! a bare call).
//!
//! Arm inventory (all verdicts pinned by venue probes,
//! docs/probes/dc22/, `sa sla check` at head 2cc12ea8):
//!   * `Some(x)`   — exactly 1 arg -> `Option<inner>`; any other
//!     arity is InvalidArgsCount with NO setError (the wrapper
//!     synthesizes "checkExpr failed at call Some ...").
//!   * `None()`    — bare only, 0 args -> `Option<infer>`.
//!   * `monitor(h)` — bare only, exactly 1 arg whose peeled type
//!     is primitive u64, else CompileError with Zig's setError
//!     texts; records the call in `monitor_calls`; returns
//!     `Chan<(u64, i32)>`.
//!   * `Ok(x)` / `Err(x)` — exactly 1 arg ->
//!     `Result<inner, infer>` / `Result<infer, inner>`.
//!   * `ptr__null` / `std__ptr__null` — 0 args, exactly 1 generic
//!     -> `*generic`. (The bare turbofish spelling `::<T>` is a
//!     SyntaxError in Zig's parser; the reachable spelling is
//!     `ptr__null<T>()` — venue probes.)
//!   * `future__pending` — 0 args, 1 generic -> `future<generic>`.
//!   * `future__defer_ready(x)` — 1 arg, 0 generics ->
//!     `future<inner>`.
//!   * `future__join2(a, b)` — both args futures of poll-scalar
//!     payloads -> `future<FuturePair<L, R>>`.
//!   * `future__pair_left/right(p)` — `FuturePair<L, R>` ->
//!     the selected side, which must be poll-scalar.
//!   * `future__select2(a, b)` — like join2 ->
//!     `future<FutureEither<L, R>>`.
//!   * `future__either_side(e)` -> u64; `future__either_left/
//!     right(e)` -> the selected side (poll-scalar gated).
//!   * `poll__ready(x)` — poll-scalar arg -> `Poll<inner>`;
//!     `poll__pending` — 0 args, 1 poll-scalar generic ->
//!     `Poll<generic>`; `poll__is_ready/is_pending(p)` -> bool;
//!     `poll__value(p)` -> the poll-scalar inner type.
//!     Poll-scalar = numeric primitive ONLY (lowering_rules
//!     `isPollScalarValueType` = `isNumericType`): floats pass,
//!     bool and String do not (venue probes).
//!   * Waker leaves — `RAW_WAKER_NEW(d, v)` (both args pointer
//!     values: `*T` or raw_ptr) -> RawWaker; `RAW_WAKER_GET_DATA/
//!     GET_VTABLE(x)` -> `*infer`; `RAW_WAKER_CLONE(x)` ->
//!     RawWaker; `WAKER_FROM_RAW(x)` / `WAKER_CLONE(x)` -> Waker;
//!     `WAKER_GET_DATA/GET_VTABLE` and the `LOCAL_WAKER_*` pair
//!     -> `*infer`; `WAKER_WILL_WAKE(a, b)` -> bool;
//!     `WAKER_WAKE/WAKE_BY_REF`, `RAW_WAKER_WAKE/WAKE_BY_REF`
//!     -> i32. Argument TYPES are otherwise unchecked (the
//!     GET/WAKE leaves accept any single argument).
//!   * `va_next_*` — see `check_va_next_call`: the six typed
//!     reads are gated on the current function being an exported
//!     extern "C" variadic definition; any other `va_next_`
//!     prefix is UndefinedVariable with Zig's setError text.
//!   * `ptr__read_volatile(p)` / `std__ptr__read_volatile(p)` —
//!     requires unsafe depth > 0 (CompileError, setError
//!     "ptr::read_volatile requires unsafe"), exactly 1 arg
//!     whose type is a pointer or borrow -> the pointee.

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    bool_ty, i32_ty, is_numeric_type, peel_borrow_pointer, prim, type_desc,
};
use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::{Primitive, TupleType, Type, UserDefined};

fn invalid_args() -> Diagnostic {
    // Most arms of this block return the bare error with NO
    // setError; the caller's wrapper synthesizes the attribution
    // text ("checkExpr failed at call {name} with {n} args
    // (error.InvalidArgsCount)"), which the venue probes show
    // verbatim. The message here follows the same shape.
    Diagnostic::new(ErrorKind::InvalidArgsCount, "InvalidArgsCount")
}

fn type_mismatch() -> Diagnostic {
    Diagnostic::new(ErrorKind::TypeMismatch, "TypeMismatch")
}

fn user_ty(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

/// lowering_rules `isPollScalarValueType` = `isNumericType`.
fn is_poll_scalar(ty: &Type) -> bool {
    is_numeric_type(ty)
}

/// lowering_rules `isPointerValueType`: a pointer type or the
/// raw_ptr primitive alias.
fn is_pointer_value(ty: &Type) -> bool {
    matches!(ty, Type::Pointer(_))
        || matches!(ty, Type::Primitive(Primitive::RawPtr))
}

/// lowering_rules `futureInnerType`: peel, then `.future`.
fn future_inner(ty: &Type) -> Option<Type> {
    match peel_borrow_pointer(ty) {
        Type::Future(inner) => Some((**inner).clone()),
        _ => None,
    }
}

/// lowering_rules `userDefinedMapTypes` for two-generic carriers
/// (`FuturePair` / `FutureEither`).
fn pair_inner_types(ty: &Type, name: &str) -> Option<(Type, Type)> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) if u.name == name && u.generics.len() == 2 => {
            Some((u.generics[0].clone(), u.generics[1].clone()))
        }
        _ => None,
    }
}

/// lowering_rules `pollInnerType` = `Poll<T>` generic inner.
fn poll_inner(ty: &Type) -> Option<Type> {
    match peel_borrow_pointer(ty) {
        Type::UserDefined(u) if u.name == "Poll" && u.generics.len() == 1 => {
            Some(u.generics[0].clone())
        }
        _ => None,
    }
}

/// The `va_next_*` read result types (type_checker.zig:141,
/// `vaNextReadType`).
fn va_next_read_type(name: &str) -> Option<Type> {
    Some(prim(match name {
        "va_next_i32" => Primitive::I32,
        "va_next_i64" => Primitive::I64,
        "va_next_u32" => Primitive::U32,
        "va_next_u64" => Primitive::U64,
        "va_next_f64" => Primitive::F64,
        "va_next_ptr" => Primitive::RawPtr,
        _ => return None,
    }))
}

impl Checker {
    /// The pre-associated bare-name block. Returns `Some(..)`
    /// when the call belongs to the block (the verdict, success
    /// or failure, is final — exactly as Zig returns from the
    /// arm); `None` when resolution must continue.
    pub fn check_pre_assoc_call(
        &mut self,
        node: &Node,
        call: &CallExpr,
    ) -> Option<CheckResult<Type>> {
        let name = call.func_name.as_str();
        let bare = call.associated_target.is_none();
        match name {
            // lowering_rules `isOptionSomeCall`: name only, the
            // associated target is NOT consulted (venue probe:
            // `foo::Some(42)` constructs an Option).
            "Some" => Some(self.check_some_call(call)),
            "None" if bare => Some(self.check_none_call(call)),
            "monitor" if bare => Some(self.check_monitor_call(node, call)),
            "Ok" => Some(self.check_ok_err_call(call, true)),
            "Err" => Some(self.check_ok_err_call(call, false)),
            "ptr__null" | "std__ptr__null" => {
                Some(self.check_ptr_null_call(call))
            }
            "future__pending" => Some(self.check_future_pending_call(call)),
            "future__defer_ready" => {
                Some(self.check_future_defer_ready_call(call))
            }
            "future__join2" => Some(self.check_future_join2_call(call, false)),
            "future__select2" => Some(self.check_future_join2_call(call, true)),
            "future__pair_left" => {
                Some(self.check_future_pair_accessor_call(call, true))
            }
            "future__pair_right" => {
                Some(self.check_future_pair_accessor_call(call, false))
            }
            "future__either_side" => {
                Some(self.check_future_either_side_call(call))
            }
            "future__either_left" => {
                Some(self.check_future_either_accessor_call(call, true))
            }
            "future__either_right" => {
                Some(self.check_future_either_accessor_call(call, false))
            }
            "poll__ready" => Some(self.check_poll_ready_call(call)),
            "poll__pending" => Some(self.check_poll_pending_call(call)),
            "poll__is_ready" | "poll__is_pending" => {
                Some(self.check_poll_is_call(call))
            }
            "poll__value" => Some(self.check_poll_value_call(call)),
            "RAW_WAKER_NEW" => Some(self.check_raw_waker_new_call(call)),
            "RAW_WAKER_GET_DATA" | "RAW_WAKER_GET_VTABLE" => {
                Some(self.check_waker_get_call(call))
            }
            "RAW_WAKER_CLONE" => {
                Some(self.check_waker_convert_call(call, "RawWaker"))
            }
            "WAKER_FROM_RAW" | "WAKER_CLONE" => {
                Some(self.check_waker_convert_call(call, "Waker"))
            }
            "WAKER_GET_DATA"
            | "WAKER_GET_VTABLE"
            | "LOCAL_WAKER_GET_DATA"
            | "LOCAL_WAKER_GET_VTABLE" => Some(self.check_waker_get_call(call)),
            "WAKER_WILL_WAKE" => Some(self.check_waker_will_wake_call(call)),
            "WAKER_WAKE"
            | "WAKER_WAKE_BY_REF"
            | "RAW_WAKER_WAKE"
            | "RAW_WAKER_WAKE_BY_REF" => Some(self.check_waker_wake_call(call)),
            "ptr__read_volatile" | "std__ptr__read_volatile" => {
                Some(self.check_read_volatile_call(call))
            }
            _ => {
                if va_next_read_type(name).is_some()
                    || name.starts_with("va_next_")
                {
                    Some(self.check_va_next_call(call))
                } else {
                    None
                }
            }
        }
    }

    /// Venue :8320 — `Some(x)` constructor arm.
    fn check_some_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        let inner = self.check_expr(&call.args[0])?;
        Ok(user_ty("Option", vec![inner]))
    }

    /// Venue :8323 — bare `None()` arm.
    fn check_none_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if !call.args.is_empty() {
            return Err(invalid_args());
        }
        Ok(user_ty("Option", vec![Type::Infer]))
    }

    /// Venue :8328 — `monitor(handle)` arm.
    fn check_monitor_call(
        &mut self,
        node: &Node,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(Diagnostic::new(
                ErrorKind::CompileError,
                "monitor(handle) expects exactly one argument: a goroutine handle (u64)"
                    .to_string(),
            ));
        }
        let h_ty = self.check_expr(&call.args[0])?;
        let h_base = peel_borrow_pointer(&h_ty);
        if !matches!(h_base, Type::Primitive(Primitive::U64)) {
            return Err(Diagnostic::new(
                ErrorKind::CompileError,
                "monitor(handle): the handle must be u64 (the value returned by `go`)"
                    .to_string(),
            ));
        }
        self.monitor_calls
            .insert(crate::types_util::node_key_of(node));
        let payload = Type::Tuple(TupleType {
            elems: vec![prim(Primitive::U64), prim(Primitive::I32)],
        });
        Ok(user_ty("Chan", vec![payload]))
    }

    /// Venue :8346/:8352 — `Ok(x)` / `Err(x)` constructor arms.
    fn check_ok_err_call(
        &mut self,
        call: &CallExpr,
        is_ok: bool,
    ) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        let inner = self.check_expr(&call.args[0])?;
        let (ok_ty, err_ty) = if is_ok {
            (inner, Type::Infer)
        } else {
            (Type::Infer, inner)
        };
        Ok(user_ty("Result", vec![ok_ty, err_ty]))
    }

    /// Venue :8358 — bare `ptr__null` arm (`isPtrNullCall`).
    fn check_ptr_null_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if !call.args.is_empty() {
            return Err(invalid_args());
        }
        if call.generics.len() != 1 {
            return Err(invalid_args());
        }
        Ok(Type::Pointer(Box::new(call.generics[0].clone())))
    }

    /// Venue :8363 — `future__pending` arm.
    fn check_future_pending_call(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        if !call.args.is_empty() {
            return Err(invalid_args());
        }
        if call.generics.len() != 1 {
            return Err(invalid_args());
        }
        Ok(Type::Future(Box::new(call.generics[0].clone())))
    }

    /// Venue :8369 — `future__defer_ready` arm.
    fn check_future_defer_ready_call(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        if !call.generics.is_empty() {
            return Err(invalid_args());
        }
        let inner = self.check_expr(&call.args[0])?;
        Ok(Type::Future(Box::new(inner)))
    }

    /// Venue :5623/:5655 — `checkFutureJoin2Call` /
    /// `checkFutureSelect2Call` (identical gates, pair vs
    /// either carrier).
    fn check_future_join2_call(
        &mut self,
        call: &CallExpr,
        select: bool,
    ) -> CheckResult<Type> {
        if call.args.len() != 2 {
            return Err(invalid_args());
        }
        if !call.generics.is_empty() {
            return Err(invalid_args());
        }
        let left_ty = self.check_expr(&call.args[0])?;
        let right_ty = self.check_expr(&call.args[1])?;
        let left_inner = future_inner(&left_ty).ok_or_else(type_mismatch)?;
        let right_inner = future_inner(&right_ty).ok_or_else(type_mismatch)?;
        if !is_poll_scalar(&left_inner) || !is_poll_scalar(&right_inner) {
            return Err(type_mismatch());
        }
        let carrier = if select { "FutureEither" } else { "FuturePair" };
        Ok(Type::Future(Box::new(user_ty(
            carrier,
            vec![left_inner, right_inner],
        ))))
    }

    /// Venue :5635 — `checkFuturePairAccessorCall`.
    fn check_future_pair_accessor_call(
        &mut self,
        call: &CallExpr,
        want_left: bool,
    ) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        if !call.generics.is_empty() {
            return Err(invalid_args());
        }
        let pair_ty = self.check_expr(&call.args[0])?;
        let (left, right) =
            pair_inner_types(&pair_ty, "FuturePair").ok_or_else(type_mismatch)?;
        let out = if want_left { left } else { right };
        if !is_poll_scalar(&out) {
            return Err(type_mismatch());
        }
        Ok(out)
    }

    /// Venue :5667 — `checkFutureEitherSideCall`.
    fn check_future_either_side_call(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        if !call.generics.is_empty() {
            return Err(invalid_args());
        }
        let either_ty = self.check_expr(&call.args[0])?;
        pair_inner_types(&either_ty, "FutureEither").ok_or_else(type_mismatch)?;
        Ok(prim(Primitive::U64))
    }

    /// Venue :5675 — `checkFutureEitherAccessorCall`.
    fn check_future_either_accessor_call(
        &mut self,
        call: &CallExpr,
        want_left: bool,
    ) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        if !call.generics.is_empty() {
            return Err(invalid_args());
        }
        let either_ty = self.check_expr(&call.args[0])?;
        let (left, right) = pair_inner_types(&either_ty, "FutureEither")
            .ok_or_else(type_mismatch)?;
        let out = if want_left { left } else { right };
        if !is_poll_scalar(&out) {
            return Err(type_mismatch());
        }
        Ok(out)
    }

    /// Venue :8430 — `poll__ready` arm.
    fn check_poll_ready_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        if !call.generics.is_empty() {
            return Err(invalid_args());
        }
        let inner = self.check_expr(&call.args[0])?;
        if !is_poll_scalar(&inner) {
            return Err(type_mismatch());
        }
        Ok(user_ty("Poll", vec![inner]))
    }

    /// Venue :8438 — `poll__pending` arm.
    fn check_poll_pending_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if !call.args.is_empty() {
            return Err(invalid_args());
        }
        if call.generics.len() != 1 {
            return Err(invalid_args());
        }
        if !is_poll_scalar(&call.generics[0]) {
            return Err(type_mismatch());
        }
        Ok(user_ty("Poll", vec![call.generics[0].clone()]))
    }

    /// Venue :8445 — `poll__is_ready` / `poll__is_pending` arm.
    fn check_poll_is_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        if !call.generics.is_empty() {
            return Err(invalid_args());
        }
        let poll_ty = self.check_expr(&call.args[0])?;
        poll_inner(&poll_ty).ok_or_else(type_mismatch)?;
        Ok(bool_ty())
    }

    /// Venue :8453 — `poll__value` arm.
    fn check_poll_value_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        if !call.generics.is_empty() {
            return Err(invalid_args());
        }
        let poll_ty = self.check_expr(&call.args[0])?;
        let inner = poll_inner(&poll_ty).ok_or_else(type_mismatch)?;
        if !is_poll_scalar(&inner) {
            return Err(type_mismatch());
        }
        Ok(inner)
    }

    /// Venue :8462 — `RAW_WAKER_NEW` arm.
    fn check_raw_waker_new_call(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        if call.args.len() != 2 {
            return Err(invalid_args());
        }
        let data_ty = self.check_expr(&call.args[0])?;
        let vtable_ty = self.check_expr(&call.args[1])?;
        if !is_pointer_value(&data_ty) || !is_pointer_value(&vtable_ty) {
            return Err(type_mismatch());
        }
        Ok(user_ty("RawWaker", Vec::new()))
    }

    /// Venue :8472/:8496 — the waker GET leaves: any single
    /// argument, result `*infer`.
    fn check_waker_get_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        self.check_expr(&call.args[0])?;
        Ok(Type::Pointer(Box::new(Type::Infer)))
    }

    /// Venue :8480/:8488 — `RAW_WAKER_CLONE` -> RawWaker,
    /// `WAKER_FROM_RAW` / `WAKER_CLONE` -> Waker.
    fn check_waker_convert_call(
        &mut self,
        call: &CallExpr,
        result_name: &str,
    ) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        self.check_expr(&call.args[0])?;
        Ok(user_ty(result_name, Vec::new()))
    }

    /// Venue :8506 — `WAKER_WILL_WAKE` arm.
    fn check_waker_will_wake_call(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        if call.args.len() != 2 {
            return Err(invalid_args());
        }
        self.check_expr(&call.args[0])?;
        self.check_expr(&call.args[1])?;
        Ok(bool_ty())
    }

    /// Venue :8515 — the waker WAKE leaves -> i32.
    fn check_waker_wake_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        self.check_expr(&call.args[0])?;
        Ok(i32_ty())
    }

    /// Venue :8525 — bare `ptr__read_volatile` arm. The
    /// associated spelling (`ptr::read_volatile`, venue :8568)
    /// shares the unsafe gate and lives in check_call's
    /// associated block.
    pub fn check_read_volatile_call(
        &mut self,
        call: &CallExpr,
    ) -> CheckResult<Type> {
        if self.unsafe_depth == 0 {
            return Err(Diagnostic::new(
                ErrorKind::CompileError,
                "ptr::read_volatile requires unsafe".to_string(),
            ));
        }
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        let ptr_ty = self.check_expr(&call.args[0])?;
        match &ptr_ty {
            Type::Pointer(inner) | Type::Borrow(inner) => {
                Ok((**inner).clone())
            }
            _ => Err(type_mismatch()),
        }
    }

    /// Venue :8388-8412 — the `va_next_*` arms. Gate order is
    /// Zig's: known-name variadic gate, then arity, then the
    /// identifier requirement, then the VaList type test. The
    /// success path needs a VaList-typed binding, which the
    /// sla-ast type surface cannot yet spell (D-C24); every
    /// reachable verdict is pinned by venue probes.
    fn check_va_next_call(&mut self, call: &CallExpr) -> CheckResult<Type> {
        let name = call.func_name.as_str();
        let Some(read_ty) = va_next_read_type(name) else {
            return Err(Diagnostic::new(
                ErrorKind::UndefinedVariable,
                format!(
                    "unknown variadic read builtin `{name}` (supported: va_next_i32, va_next_i64, va_next_u32, va_next_u64, va_next_f64, va_next_ptr)"
                ),
            ));
        };
        if !self.current_fn_is_variadic {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "{name} may only be used inside an exported extern \"C\" variadic definition"
                ),
            ));
        }
        if call.args.len() != 1 {
            return Err(invalid_args());
        }
        if !matches!(&call.args[0], Node::Identifier(_)) {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "{name} argument must be the VaList parameter binding itself (VaList is not copyable)"
                ),
            ));
        }
        let list_ty = self.check_expr(&call.args[0])?;
        // Zig: `list_ty` must be primitive va_list, and the arm
        // then returns `read_ty`. sla-ast has no VaList primitive
        // (D-C24), so every spellable type lands on Zig's own
        // mismatch message instead; `read_ty` becomes the result
        // the day the VaList surface lands.
        let _ = &read_ty;
        Err(Diagnostic::new(
            ErrorKind::TypeMismatch,
            format!(
                "{name} argument must be the VaList parameter, got {}",
                type_desc(&list_ty)
            ),
        ))
    }
}
