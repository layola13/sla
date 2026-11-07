//! End-to-end tests: SLA source text -> `sla_parser::parse_program`
//! -> `Checker::check_program`. This is the shape whose absence let
//! INT-1 through: the hand-built-tree tests in tests.rs never
//! exercised a plain `return` function arriving from the parser.
//!
//! Every expectation below is grounded, not guessed:
//!   * Zig source: `checkFunc` tail check (type_checker.zig:
//!     2596-2604) and the `return_stmt` arm (type_checker.zig:2991
//!     onward: value checked against ret_ty, mismatch =
//!     TypeMismatch "TypeMismatch in return: ...").
//!   * Venue truth (2026-11-07, venue 3, real plugin at current
//!     head 6002b386, `sa sla check` per probe): p1 return rc=0,
//!     p2 if-both-return rc=0, p3 tail-if rc=0, p4 tail-expr rc=0,
//!     p5 tail-mismatch rc=1 (TypeMismatch in function tail
//!     expression: expected i32, actual boolean), p6 no-tail-let
//!     rc=0, p7 return-mismatch rc=1 (TypeMismatch in return),
//!     p8 empty-body rc=0.

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

/// INT-1 minimal repro, verbatim docs/probes/e2e_c.sla: a trailing
/// `return` statement satisfies a non-void function. Zig's tail
/// check skips non-`expr_stmt` tails entirely; the return arm has
/// already checked the value's type.
#[test]
fn e2e_c_explicit_return_passes() {
    let source = "fn one() -> i32 {\n    return 1;\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// `return` in both `if` branches (fixture 07 `fact` shape): the
/// tail is an `expr_stmt` wrapping an if whose then/else blocks
/// both terminate, so `stmtTerminates(last)` holds and Zig skips
/// the tail check. (The pre-fix port typed that if as void and
/// reported a return-type mismatch.)
#[test]
fn if_both_arms_return_passes() {
    let source = "fn pick(flag: bool) -> i32 {\n    if flag { return 1; } else { return 2; };\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Tail expression: a non-terminating final expression statement
/// IS the function's value (fixture 03 `max` shape); Zig checks
/// its recorded type against the return type — i32 vs i32 passes.
#[test]
fn tail_if_expression_passes() {
    let source =
        "fn max(a: i32, b: i32) -> i32 {\n    if a > b { a } else { b }\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Tail expression, plain arithmetic form (no trailing semicolon).
#[test]
fn tail_expression_passes() {
    let source = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// The tail check still bites where Zig has one: a boolean tail in
/// an i32 function is a TypeMismatch (venue p5, exact Zig message
/// family "TypeMismatch in function tail expression").
#[test]
fn tail_type_mismatch_rejected() {
    let source = "fn bad() -> i32 {\n    true\n}\n";
    let err = check_source(source).expect_err("boolean tail must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

/// The `return` arm's own type check is untouched by the INT-1
/// fix: returning a boolean from an i32 function still fails
/// (venue p7, TypeMismatch in return).
#[test]
fn return_type_mismatch_rejected() {
    let source = "fn bad() -> i32 {\n    return true;\n}\n";
    let err = check_source(source).expect_err("return mismatch must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

/// Zig quirk, pinned deliberately (do NOT "fix" this into a
/// missing-return error — that invention was INT-1): a non-void
/// function whose last statement is not an expression statement
/// passes the checker, because `checkFunc` only ever inspects an
/// `expr_stmt` tail. Venue p6 (`let` tail) rc=0.
#[test]
fn non_expression_tail_passes_per_zig() {
    let source = "fn f() -> i32 {\n    let x = 1;\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Same quirk, empty body: Zig guards the tail check with
/// `func.body.len > 0`, so an empty non-void body passes the
/// checker too. Venue p8 rc=0.
#[test]
fn empty_body_passes_per_zig() {
    let source = "fn f() -> i32 {\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}
