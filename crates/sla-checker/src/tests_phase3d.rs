//! Phase 3 (INT21) tests: the checker name-special call arms
//! (INT2-1) — `vec` / `len` / `str_eq` / `format` / `hash` /
//! `debug` / `println` / `catch_unwind`, mirrored from the venue
//! head's bare-name block (type_checker.zig:8805-8895).
//!
//! Every expectation is pinned by a venue probe (2026-11-07,
//! venue 3, real plugin at head 6002b386, `sa sla check`,
//! /tmp/int21/p_*.sla):
//!   println: arity 0/1/2/4 rc=0; user-fn shadow rc=0; String
//!     argument not consumed (twice) rc=0; undefined argument
//!     rc=1 UndefinedVariable.
//!   str_eq: ("a","a") rc=0; String vars twice rc=0; [u8; 3]
//!     params rc=0; (1,2) rc=1 TypeMismatch; arity 1/3 rc=1
//!     InvalidArgsCount.
//!   len: ("abc") and (42) rc=0 (argument type NOT inspected);
//!     String twice rc=0; user-fn shadow rc=0; arity 0/2 rc=1
//!     InvalidArgsCount "len call arity mismatch: {n}".
//!   format: ("{}", 42) rc=0 and result is string-like; arity 0
//!     rc=1 InvalidArgsCount; variable format string rc=1
//!     InvalidArgsCount.
//!   hash: (42)/(true) rc=0 -> u64; (1.5) rc=1 TypeMismatch;
//!     ("a") rc=1 TypeMismatch; String value rc=1 TypeMismatch;
//!     arity 2 rc=1 InvalidArgsCount; @derive(hash, debug)
//!     struct rc=0; no-derive struct rc=1 TypeMismatch;
//!     derive struct with a String field rc=1 TypeMismatch.
//!   debug: (42)/(1.5) rc=0 -> String; ("a") rc=1 TypeMismatch;
//!     String value rc=1 TypeMismatch; no-derive struct rc=1.
//!   vec: (1,2,3) rc=0 -> Vec (len of it rc=0); () rc=1
//!     InvalidArgsCount "vec requires at least one element";
//!     (1, "a") rc=1 TypeMismatch.
//!   catch_unwind: std::panic::catch_unwind(|| panic(1)) rc=0
//!     (rosetta 173 demo checks rc=0 whole-file); bare
//!     std__panic__catch_unwind rc=0; non-panic body rc=1
//!     TypeMismatch "catch_unwind currently only supports
//!     direct panic bodies"; arity 2 rc=1 InvalidArgsCount.

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

// -- println --------------------------------------------------------------

#[test]
fn println_any_arity_passes() {
    expect_ok(
        "fn main() -> i32 { println(); println(1); println(1, 2); println(\"{}\", 1, 2, 3); return 0; }",
    );
}

#[test]
fn println_shadows_user_function() {
    // The arm sits before user-function resolution: the user's
    // 1-parameter `println` cannot make the 2-arg call fail.
    expect_ok(
        "fn println(x: i32) -> i32 { return x; }\nfn main() -> i32 { println(1, 2); return 0; }",
    );
}

#[test]
fn println_does_not_consume_string_argument() {
    expect_ok(
        "fn main() -> i32 { let s = format(\"x\"); println(s); println(s); return 0; }",
    );
}

#[test]
fn println_propagates_argument_errors() {
    expect_err(
        "fn main() -> i32 { println(nope); return 0; }",
        ErrorKind::UndefinedVariable,
    );
}

// -- str_eq ---------------------------------------------------------------

#[test]
fn str_eq_literals_and_strings_pass() {
    expect_ok(
        "fn main() -> i32 { if str_eq(\"a\", \"a\") { return 1; }; return 0; }",
    );
    // String bindings are checked, not consumed: reusable.
    expect_ok(
        "fn main() -> i32 { let a = format(\"x\"); let b = format(\"y\"); if str_eq(a, b) { return 1; }; if str_eq(a, b) { return 2; }; return 0; }",
    );
}

#[test]
fn str_eq_array_arguments_pass() {
    // isStringLikeType: any array qualifies (venue probe).
    expect_ok(
        "fn f(a: [u8; 3]) -> bool { return str_eq(a, a); }\nfn main() -> i32 { return 0; }",
    );
}

#[test]
fn str_eq_rejects_non_string_like() {
    expect_err(
        "fn main() -> i32 { if str_eq(1, 2) { return 1; }; return 0; }",
        ErrorKind::TypeMismatch,
    );
}

#[test]
fn str_eq_arity_is_exactly_two() {
    expect_err(
        "fn main() -> i32 { if str_eq(\"a\") { return 1; }; return 0; }",
        ErrorKind::InvalidArgsCount,
    );
    expect_err(
        "fn main() -> i32 { if str_eq(\"a\", \"a\", \"a\") { return 1; }; return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

// -- len ------------------------------------------------------------------

#[test]
fn len_accepts_any_single_argument_type() {
    // The Zig arm checks the expression and returns usize
    // WITHOUT inspecting its type: len(42) is legal.
    expect_ok(
        "fn main() -> i32 { let n: usize = len(\"abc\"); let m: usize = len(42); return 0; }",
    );
    expect_ok(
        "fn main() -> i32 { let s = format(\"x\"); let n: usize = len(s); let m: usize = len(s); return 0; }",
    );
}

#[test]
fn len_arity_mismatch_reports_count() {
    let err = expect_err(
        "fn main() -> i32 { let n: usize = len(); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
    assert!(err.message.contains("len call arity mismatch: 0"), "{err:?}");
    let err = expect_err(
        "fn main() -> i32 { let n: usize = len(1, 2); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
    assert!(err.message.contains("len call arity mismatch: 2"), "{err:?}");
}

#[test]
fn len_shadows_user_function() {
    expect_ok(
        "fn len(a: i32, b: i32) -> i32 { return a + b; }\nfn main() -> i32 { let n: usize = len(1); return 0; }",
    );
}

// -- format ---------------------------------------------------------------

#[test]
fn format_returns_string() {
    expect_ok(
        "fn main() -> i32 { let s = format(\"{}\", 42); if str_eq(s, \"42\") { return 1; }; return 0; }",
    );
}

#[test]
fn format_requires_string_literal_first_argument() {
    expect_err(
        "fn main() -> i32 { let s = format(); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
    // A string-TYPED variable is not enough: the node must be a
    // string literal (venue probe p_format_varfmt, rc=1).
    expect_err(
        "fn main() -> i32 { let f = \"{}\"; let s = format(f, 42); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

// -- hash / debug -----------------------------------------------------------

#[test]
fn hash_accepts_hashable_primitives() {
    expect_ok(
        "fn main() -> i32 { let h: u64 = hash(42); let h2: u64 = hash(true); return 0; }",
    );
}

#[test]
fn hash_rejects_float_and_raw_ptr() {
    expect_err(
        "fn main() -> i32 { let h: u64 = hash(1.5); return 0; }",
        ErrorKind::TypeMismatch,
    );
    // String literals type as raw_ptr, which is not hashable.
    expect_err(
        "fn main() -> i32 { let h: u64 = hash(\"a\"); return 0; }",
        ErrorKind::TypeMismatch,
    );
    expect_err(
        "fn main() -> i32 { let h: u64 = hash(1, 2); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

#[test]
fn hash_rejects_string_values() {
    // String is a std owner type: the derive base decides false
    // before any struct gate (venue probe p_hash_stringvar).
    expect_err(
        "fn main() -> i32 { let s = format(\"x\"); let h: u64 = hash(s); return 0; }",
        ErrorKind::TypeMismatch,
    );
}

#[test]
fn debug_accepts_copy_value_primitives_including_float() {
    expect_ok(
        "fn main() -> i32 { let s = debug(42); if str_eq(s, \"42\") { return 1; }; return 0; }",
    );
    expect_ok("fn main() -> i32 { let s = debug(1.5); return 0; }");
}

#[test]
fn debug_rejects_raw_ptr_and_string_values() {
    expect_err(
        "fn main() -> i32 { let s = debug(\"a\"); return 0; }",
        ErrorKind::TypeMismatch,
    );
    expect_err(
        "fn main() -> i32 { let s = format(\"x\"); let d = debug(s); return 0; }",
        ErrorKind::TypeMismatch,
    );
}

#[test]
fn hash_debug_derive_gated_structs() {
    expect_ok(
        "@derive(hash, debug)\nstruct Ent { id: i32, gen: i32 }\nfn main() -> i32 { let a = Ent { id: 1, gen: 2 }; let h: u64 = hash(a); let s = debug(a); return 0; }",
    );
    expect_err(
        "struct Plain { id: i32 }\nfn main() -> i32 { let a = Plain { id: 1 }; let h: u64 = hash(a); return 0; }",
        ErrorKind::TypeMismatch,
    );
    expect_err(
        "struct Plain { id: i32 }\nfn main() -> i32 { let a = Plain { id: 1 }; let s = debug(a); return 0; }",
        ErrorKind::TypeMismatch,
    );
    // The derive gate opens, then field recursion closes it: a
    // String field is not hashable (venue probe).
    expect_err(
        "@derive(hash)\nstruct HasStr { s: String, n: i32 }\nfn main() -> i32 { let a = HasStr { s: format(\"x\"), n: 1 }; let h: u64 = hash(a); return 0; }",
        ErrorKind::TypeMismatch,
    );
}

// -- vec ------------------------------------------------------------------

#[test]
fn vec_constructor_types_elements() {
    expect_ok(
        "fn main() -> i32 { let v = vec(1, 2, 3); let n: usize = len(v); return 0; }",
    );
}

#[test]
fn vec_requires_uniform_elements() {
    let err = expect_err(
        "fn main() -> i32 { let v = vec(); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
    assert!(
        err.message.contains("vec requires at least one element"),
        "{err:?}"
    );
    expect_err(
        "fn main() -> i32 { let v = vec(1, \"a\"); return 0; }",
        ErrorKind::TypeMismatch,
    );
}

// -- catch_unwind -----------------------------------------------------------

#[test]
fn catch_unwind_accepts_direct_panic_bodies() {
    expect_ok(
        "fn main() -> i32 { let r = std::panic::catch_unwind(|| panic(1)); return 0; }",
    );
    expect_ok(
        "fn main() -> i32 { let r = std__panic__catch_unwind(|| panic(1)); return 0; }",
    );
}

#[test]
fn catch_unwind_rejects_non_panic_bodies_and_arity() {
    let err = expect_err(
        "fn main() -> i32 { let r = std::panic::catch_unwind(|| 42); return 0; }",
        ErrorKind::TypeMismatch,
    );
    assert!(
        err.message
            .contains("catch_unwind currently only supports direct panic bodies"),
        "{err:?}"
    );
    expect_err(
        "fn main() -> i32 { let r = std::panic::catch_unwind(|| panic(1), || panic(2)); return 0; }",
        ErrorKind::InvalidArgsCount,
    );
}

// -- the five fixtures, verbatim ------------------------------------------

#[test]
fn fixture_01_hello_checks() {
    expect_ok(include_str!("../../../fixtures/01_hello.sla"));
}

#[test]
fn fixture_02_arith_checks() {
    expect_ok(include_str!("../../../fixtures/02_arith.sla"));
}

#[test]
fn fixture_03_if_else_checks() {
    expect_ok(include_str!("../../../fixtures/03_if_else.sla"));
}

#[test]
fn fixture_04_while_sum_checks() {
    expect_ok(include_str!("../../../fixtures/04_while_sum.sla"));
}

#[test]
fn fixture_07_functions_checks() {
    expect_ok(include_str!("../../../fixtures/07_functions.sla"));
}
