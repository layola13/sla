//! Phase 4 (D-C22) tests: the pre-associated bare-name block
//! (check_preassoc.rs) — `Some`/`None`/`Ok`/`Err`, `monitor`,
//! `ptr__null`, the `future__*` / `poll__*` runtime leaves, the
//! waker leaves, `va_next_*`, and `ptr__read_volatile`.
//!
//! Every expectation is pinned by a venue probe (2026-11-07,
//! Colab box, real plugin at sla head 2cc12ea8, `sa sla check`,
//! 90 probes in docs/probes/dc22/):
//!   Some: (42)->Option rc=0 incl. return position; arity 0/2
//!     rc=1 InvalidArgsCount; user `fn Some(a, b)` cannot shadow
//!     (1-arg rc=0, 2-arg still rc=1); `foo::Some(42)` rc=0 —
//!     the predicate ignores the associated target.
//!   None(): rc=0 -> Option; None(1) rc=1 InvalidArgsCount.
//!   Ok/Err: rc=0 -> Result in return position; arity 0/2 rc=1
//!     InvalidArgsCount; `foo::Ok(42)` rc=0.
//!   monitor: (u64) rc=0 -> Chan<(u64, i32)> (return-position
//!     mismatch rc=1 TypeMismatch "actual Chan"); (i32) rc=1
//!     CompileError "the handle must be u64"; arity 0/2 rc=1
//!     CompileError "expects exactly one argument"; (&u64) rc=0
//!     (borrow peel); `foo::monitor(h)` rc=1 UndefinedVariable.
//!   ptr__null: `<i32>` spelling rc=0 -> *i32 (the `::<i32>`
//!     spelling is a SyntaxError in Zig's parser, so the checker
//!     generics arrive via `ptr__null<i32>()`); no generic rc=1
//!     InvalidArgsCount.
//!   future__: pending<i32> rc=0 -> future<i32>; no generic rc=1
//!     InvalidArgsCount; defer_ready(42) rc=0; join2 of two
//!     scalar futures rc=0 -> future<FuturePair<..>> (return
//!     position); non-future / non-scalar-payload rc=1
//!     TypeMismatch; arity 1 rc=1 InvalidArgsCount; pair_left/
//!     right rc=0 typed; non-pair / non-scalar side rc=1
//!     TypeMismatch; select2 rc=0 -> future<FutureEither<..>>;
//!     either_side rc=0 -> u64; either_left/right rc=0 typed;
//!     non-either rc=1 TypeMismatch.
//!   poll__: ready(42) rc=0 -> Poll<i32>; ready(1.5) rc=0
//!     (floats are numeric); ready(String) rc=1 TypeMismatch;
//!     pending<i32> rc=0; pending<String> rc=1 TypeMismatch;
//!     is_ready/is_pending rc=0 -> bool; non-poll rc=1
//!     TypeMismatch; value rc=0 -> inner; Poll<String> value
//!     rc=1 TypeMismatch.
//!   waker: RAW_WAKER_NEW(ptr, ptr) rc=0 -> RawWaker; (1, 2)
//!     rc=1 TypeMismatch; arity 1 rc=1 InvalidArgsCount;
//!     GET_DATA rc=0 -> *u8 in return position; CLONE rc=0 ->
//!     RawWaker; WAKER_FROM_RAW / WAKER_CLONE rc=0 -> Waker;
//!     WAKER_GET_VTABLE / LOCAL_WAKER_GET_DATA rc=0;
//!     WILL_WAKE rc=0 -> bool, arity 1 rc=1; WAKE / WAKE_BY_REF /
//!     RAW_WAKER_WAKE(_BY_REF) rc=0 -> i32.
//!   read_volatile: inside unsafe rc=0 -> pointee; outside
//!     unsafe rc=1 CompileError "ptr::read_volatile requires
//!     unsafe"; std__ spelling rc=0; non-pointer arg rc=1
//!     TypeMismatch.
//!   va_next: outside a variadic definition rc=1 TypeMismatch
//!     "may only be used inside an exported extern \"C\"
//!     variadic definition"; unknown `va_next_bool` rc=1
//!     UndefinedVariable with the supported-list text. The
//!     in-variadic verdicts (arity / identifier / VaList-type)
//!     are pinned here by driving `current_fn_is_variadic`
//!     directly — the parsed-program surface cannot spell a
//!     variadic definition yet (D-C24).

use crate::checker::Checker;
use crate::diag::{Diagnostic, ErrorKind};
use sla_ast::node::Node;

fn check_source(source: &str) -> Result<(), Diagnostic> {
    let root = sla_parser::parse_program(source).expect("source must parse");
    let Node::Program(program) = &root else {
        panic!("parser root must be a Program node");
    };
    let mut checker = Checker::new();
    checker.check_program(program)
}

fn expect_ok(source: &str) {
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

fn expect_err(source: &str, kind: ErrorKind) -> Diagnostic {
    let err = check_source(source).expect_err("source must fail check");
    assert_eq!(err.kind, kind, "{err:?}");
    err
}

// -- Some / None / Ok / Err -------------------------------------------------

#[test]
fn some_constructs_option() {
    expect_ok(
        "fn f() -> Option<i32> { return Some(42); }\nfn main() -> i32 { return 0; }",
    );
}

#[test]
fn some_arity_is_exactly_one() {
    expect_err(
        "fn main() -> i32 { let o = Some(); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
    expect_err(
        "fn main() -> i32 { let o = Some(1, 2); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

#[test]
fn some_cannot_be_shadowed_by_user_function() {
    expect_ok(
        "fn Some(a: i32, b: i32) -> i32 { return a + b; }\nfn f() -> Option<i32> { return Some(42); }\nfn main() -> i32 { return 0; }",
    );
    // The user's 2-arg Some does NOT become reachable either:
    // the arm pre-empts and rejects the arity.
    expect_err(
        "fn Some(a: i32, b: i32) -> i32 { return a + b; }\nfn main() -> i32 { let o = Some(1, 2); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

#[test]
fn some_ignores_associated_target() {
    // Zig's isOptionSomeCall consults the name only (venue
    // probe r_some_assoc / r_ok_assoc, rc=0).
    expect_ok("fn main() -> i32 { let o = foo::Some(42); return 0; }");
    expect_ok("fn main() -> i32 { let r = foo::Ok(42); return 0; }");
}

#[test]
fn none_call_constructs_option() {
    expect_ok(
        "fn f() -> Option<i32> { return None(); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let o = None(1); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

#[test]
fn ok_err_construct_result() {
    expect_ok(
        "fn f() -> Result<i32, i32> { return Ok(7); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f() -> Result<i32, i32> { return Err(7); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let r = Ok(); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
    expect_err(
        "fn main() -> i32 { let r = Err(1, 2); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

// -- monitor ----------------------------------------------------------------

#[test]
fn monitor_accepts_u64_handle() {
    expect_ok(
        "fn f(h: u64) -> i32 { let c = monitor(h); return 0; }\nfn main() -> i32 { return 0; }",
    );
    // Borrow peel: &u64 is accepted (venue probe r_monitor_borrow).
    expect_ok(
        "fn f(h: u64) -> i32 { let c = monitor(&h); return 0; }\nfn main() -> i32 { return 0; }",
    );
}

#[test]
fn monitor_result_is_chan_of_u64_i32_tuple() {
    expect_ok(
        "fn f(h: u64) -> Chan<(u64, i32)> { return monitor(h); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn f(h: u64) -> i32 { return monitor(h); }\nfn main() -> i32 { return 0; }",
        ErrorKind::TypeMismatch,
    );
}

#[test]
fn monitor_rejects_non_u64_handle() {
    let err = expect_err(
        "fn f(h: i32) -> i32 { let c = monitor(h); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::CompileError,
    );
    assert!(
        err.message.contains("the handle must be u64"),
        "{err:?}"
    );
}

#[test]
fn monitor_arity_is_compile_error_with_text() {
    let err = expect_err(
        "fn main() -> i32 { let c = monitor(); return 0; }",
        ErrorKind::CompileError,
    );
    assert!(
        err.message.contains("monitor(handle) expects exactly one argument"),
        "{err:?}"
    );
    expect_err(
        "fn f(h: u64) -> i32 { let c = monitor(h, h); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::CompileError,
    );
}

#[test]
fn monitor_is_bare_only() {
    expect_err(
        "fn f(h: u64) -> i32 { let c = foo::monitor(h); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::UndefinedVariable,
    );
}

// -- ptr__null ----------------------------------------------------------------

#[test]
fn ptr_null_with_generic_is_pointer() {
    expect_ok(
        "fn f() -> *i32 { return ptr__null<i32>(); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f() -> *i32 { return std__ptr__null<i32>(); }\nfn main() -> i32 { return 0; }",
    );
}

#[test]
fn ptr_null_requires_exactly_one_generic() {
    expect_err(
        "fn main() -> i32 { let p = ptr__null(); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

// -- future__ -----------------------------------------------------------------

#[test]
fn future_pending_with_generic() {
    expect_ok(
        "fn f() -> future<i32> { return future__pending<i32>(); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let f = future__pending(); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

#[test]
fn future_defer_ready_wraps_value() {
    expect_ok(
        "fn f() -> future<i32> { return future__defer_ready(42); }\nfn main() -> i32 { return 0; }",
    );
}

#[test]
fn future_join2_builds_pair_future() {
    expect_ok(
        "fn f(a: future<i32>, b: future<u64>) -> future<FuturePair<i32, u64>> { return future__join2(a, b); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let j = future__join2(1, 2); return 0; }",
        ErrorKind::TypeMismatch,
    );
    // Non-scalar (String) payloads are rejected.
    expect_err(
        "fn f(a: future<String>, b: future<i32>) -> i32 { let j = future__join2(a, b); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::TypeMismatch,
    );
    expect_err(
        "fn f(a: future<i32>) -> i32 { let j = future__join2(a); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

#[test]
fn future_pair_accessors_project_sides() {
    expect_ok(
        "fn f(p: FuturePair<i32, u64>) -> i32 { return future__pair_left(p); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(p: FuturePair<i32, u64>) -> u64 { return future__pair_right(p); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let x = future__pair_left(42); return 0; }",
        ErrorKind::TypeMismatch,
    );
    expect_err(
        "fn f(p: FuturePair<String, i32>) -> i32 { let x = future__pair_left(p); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::TypeMismatch,
    );
}

#[test]
fn future_select2_and_either_accessors() {
    expect_ok(
        "fn f(a: future<i32>, b: future<u64>) -> future<FutureEither<i32, u64>> { return future__select2(a, b); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(e: FutureEither<i32, u64>) -> u64 { return future__either_side(e); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(e: FutureEither<i32, u64>) -> i32 { return future__either_left(e); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(e: FutureEither<i32, u64>) -> u64 { return future__either_right(e); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let x = future__either_side(42); return 0; }",
        ErrorKind::TypeMismatch,
    );
    expect_err(
        "fn main() -> i32 { let s = future__select2(1, 2); return 0; }",
        ErrorKind::TypeMismatch,
    );
}

// -- poll__ -------------------------------------------------------------------

#[test]
fn poll_ready_requires_numeric_scalar() {
    expect_ok(
        "fn f() -> Poll<i32> { return poll__ready(42); }\nfn main() -> i32 { return 0; }",
    );
    // Floats ARE numeric: Poll<f64> is fine (venue probe).
    expect_ok(
        "fn f() -> Poll<f64> { return poll__ready(1.5); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn f(s: String) -> i32 { let p = poll__ready(s); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::TypeMismatch,
    );
}

#[test]
fn poll_pending_uses_generic_scalar() {
    expect_ok(
        "fn f() -> Poll<i32> { return poll__pending<i32>(); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let p = poll__pending<String>(); return 0; }",
        ErrorKind::TypeMismatch,
    );
}

#[test]
fn poll_predicates_and_value() {
    expect_ok(
        "fn f(p: Poll<i32>) -> bool { return poll__is_ready(p); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(p: Poll<i32>) -> bool { return poll__is_pending(p); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(p: Poll<i32>) -> i32 { return poll__value(p); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let b = poll__is_ready(42); return 0; }",
        ErrorKind::TypeMismatch,
    );
    expect_err(
        "fn main() -> i32 { let x = poll__value(42); return 0; }",
        ErrorKind::TypeMismatch,
    );
    expect_err(
        "fn f(p: Poll<String>) -> i32 { let x = poll__value(p); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::TypeMismatch,
    );
}

// -- waker leaves ---------------------------------------------------------------

#[test]
fn raw_waker_new_requires_pointer_values() {
    expect_ok(
        "fn f(d: ptr, v: ptr) -> RawWaker { return RAW_WAKER_NEW(d, v); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let r = RAW_WAKER_NEW(1, 2); return 0; }",
        ErrorKind::TypeMismatch,
    );
    expect_err(
        "fn f(d: ptr) -> i32 { let r = RAW_WAKER_NEW(d); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

#[test]
fn waker_get_leaves_return_infer_pointer() {
    expect_ok(
        "fn f(r: RawWaker) -> *u8 { return RAW_WAKER_GET_DATA(r); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(w: Waker) -> i32 { let d = WAKER_GET_VTABLE(w); return 0; }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(w: Waker) -> i32 { let d = LOCAL_WAKER_GET_DATA(w); return 0; }\nfn main() -> i32 { return 0; }",
    );
}

#[test]
fn waker_convert_clone_and_wake_leaves() {
    expect_ok(
        "fn f(r: RawWaker) -> RawWaker { return RAW_WAKER_CLONE(r); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(r: RawWaker) -> Waker { return WAKER_FROM_RAW(r); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(w: Waker) -> Waker { return WAKER_CLONE(w); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(a: Waker, b: Waker) -> bool { return WAKER_WILL_WAKE(a, b); }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn f(a: Waker) -> i32 { let b = WAKER_WILL_WAKE(a); return 0; }\nfn main() -> i32 { return 0; }",
        ErrorKind::InvalidArgsCount,
    );
    expect_ok(
        "fn f(w: Waker) -> i32 { return WAKER_WAKE(w); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(w: Waker) -> i32 { return WAKER_WAKE_BY_REF(w); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(r: RawWaker) -> i32 { return RAW_WAKER_WAKE(r); }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(r: RawWaker) -> i32 { return RAW_WAKER_WAKE_BY_REF(r); }\nfn main() -> i32 { return 0; }",
    );
}

// -- ptr__read_volatile ---------------------------------------------------------

#[test]
fn read_volatile_requires_unsafe() {
    let err = expect_err(
        "fn f(p: *i32) -> i32 { return ptr__read_volatile(p); }\nfn main() -> i32 { return 0; }",
        ErrorKind::CompileError,
    );
    assert!(
        err.message.contains("ptr::read_volatile requires unsafe"),
        "{err:?}"
    );
}

#[test]
fn read_volatile_inside_unsafe_returns_pointee() {
    expect_ok(
        "fn f(p: *i32) -> i32 { let v = unsafe { ptr__read_volatile(p) }; return v; }\nfn main() -> i32 { return 0; }",
    );
    expect_ok(
        "fn f(p: *i32) -> i32 { let v = unsafe { std__ptr__read_volatile(p) }; return v; }\nfn main() -> i32 { return 0; }",
    );
    expect_err(
        "fn main() -> i32 { let v = unsafe { ptr__read_volatile(42) }; return 0; }",
        ErrorKind::TypeMismatch,
    );
}

// -- va_next --------------------------------------------------------------------

#[test]
fn va_next_outside_variadic_is_type_mismatch() {
    let err = expect_err(
        "fn main() -> i32 { let a = va_next_i32(42); return 0; }",
        ErrorKind::TypeMismatch,
    );
    assert!(
        err.message
            .contains("may only be used inside an exported extern \"C\" variadic definition"),
        "{err:?}"
    );
}

#[test]
fn va_next_unknown_prefix_is_undefined_variable() {
    let err = expect_err(
        "fn main() -> i32 { let a = va_next_bool(42); return 0; }",
        ErrorKind::UndefinedVariable,
    );
    assert!(
        err.message.contains("unknown variadic read builtin `va_next_bool`"),
        "{err:?}"
    );
}

fn check_source_variadic(source: &str) -> Result<(), Diagnostic> {
    let root = sla_parser::parse_program(source).expect("source must parse");
    let Node::Program(program) = &root else {
        panic!("parser root must be a Program node");
    };
    let mut checker = Checker::new();
    // The parsed surface cannot spell a variadic definition yet
    // (D-C24); drive the Zig gate state directly.
    checker.current_fn_is_variadic = true;
    checker.check_program(program)
}

#[test]
fn va_next_inside_variadic_gates() {
    // Arity (venue probe p_va_arity2, rc=1 InvalidArgsCount).
    let err = check_source_variadic(
        "fn f(fixed: i32, other: i32) -> i64 { let a = va_next_i32(other, other); return fixed as i64; }\nfn main() -> i32 { return 0; }",
    )
    .expect_err("arity must fail");
    assert_eq!(err.kind, ErrorKind::InvalidArgsCount, "{err:?}");
    // Non-identifier argument (venue probe p_va_nonident_arg).
    let err = check_source_variadic(
        "fn f(fixed: i32) -> i64 { let a = va_next_i32(42); return fixed as i64; }\nfn main() -> i32 { return 0; }",
    )
    .expect_err("literal argument must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch, "{err:?}");
    assert!(
        err.message.contains("must be the VaList parameter binding itself"),
        "{err:?}"
    );
    // Identifier of the wrong type (venue probe p_va_wrong_binding:
    // "must be the VaList parameter, got i32").
    let err = check_source_variadic(
        "fn f(fixed: i32) -> i64 { let a = va_next_i32(fixed); return fixed as i64; }\nfn main() -> i32 { return 0; }",
    )
    .expect_err("non-VaList binding must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch, "{err:?}");
    assert!(
        err.message.contains("must be the VaList parameter, got i32"),
        "{err:?}"
    );
}
