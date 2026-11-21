//! DEF-RUST-CONST-NOT-ENFORCED (task 74) tests: `check_assign`
//! never consulted the scope's `is_const` slot, so const
//! reassignment passed check/build with rc=0 and the emitted .sa
//! contained the write-back — while the Zig plugin rejects the
//! same forms at type_checker.zig:8125 ("let bindings can be
//! reassigned and indexed; const bindings cannot").
//!
//! Every negative case below FAILS on the pre-fix checker
//! (rc=0, no diagnostic) and PASSES after the fix (rc=1,
//! ErrorKind::ConstReassign). Controls pin the boundary: `let`
//! stays mutable, function parameters stay mutable, reading a
//! const stays legal, and a shadowing `let` may be assigned.

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

// -- the three const forms (task 74) ---------------------------------------

#[test]
fn const_reassign_local_rejected() {
    let err = expect_err(
        "fn main() -> i32 { const x = 1; x = 2; return x; }",
        ErrorKind::ConstReassign,
    );
    assert!(err.message.contains("`x`"), "{err:?}");
}

#[test]
fn const_reassign_toplevel_rejected() {
    expect_err(
        "const X = 1;\nfn main() -> i32 { X = 2; return X; }",
        ErrorKind::ConstReassign,
    );
}

#[test]
fn const_reassign_typed_rejected() {
    expect_err(
        "fn main() -> i32 { const x: i32 = 1; x = 2; return x; }",
        ErrorKind::ConstReassign,
    );
}

// -- the same violation one level down -------------------------------------

#[test]
fn const_struct_field_reassign_rejected() {
    expect_err(
        "struct S { a: i32 }\nfn main() -> i32 { const s = S { a: 1 }; s.a = 2; return s.a; }",
        ErrorKind::ConstReassign,
    );
}

#[test]
fn const_array_index_reassign_rejected() {
    expect_err(
        "fn main() -> i32 { const a = [1, 2, 3]; a[0] = 9; return a[0]; }",
        ErrorKind::ConstReassign,
    );
}

// -- controls: the boundary stays where it was ------------------------------

#[test]
fn let_reassign_stays_legal() {
    expect_ok("fn main() -> i32 { let x = 1; x = 2; return x; }");
}

#[test]
fn toplevel_const_read_stays_legal() {
    expect_ok("const X = 1;\nfn main() -> i32 { return X; }");
}

#[test]
fn param_assign_stays_legal() {
    expect_ok("fn f(x: i32) -> i32 { x = 2; return x; }\nfn main() -> i32 { return f(1); }");
}

#[test]
fn let_shadowing_const_may_be_assigned() {
    expect_ok(
        "const x = 1;\nfn main() -> i32 { let x = 2; x = 3; return x; }",
    );
}

#[test]
fn let_struct_field_reassign_stays_legal() {
    expect_ok(
        "struct S { a: i32 }\nfn main() -> i32 { let s = S { a: 1 }; s.a = 2; return s.a; }",
    );
}
