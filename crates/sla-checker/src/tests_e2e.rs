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

// -- stage 5b checker-plans -------------------------------------------
// Helper: parse + check, keeping the parsed tree and the Checker so
// the recorded per-arm exit cleanups can be read back (the table is
// keyed by node address into this tree).
fn check_source_keep(source: &str) -> (Node, Checker, Result<(), Diagnostic>) {
    let root = sla_parser::parse_program(source).expect("source must parse");
    let Node::Program(program) = &root else {
        panic!("parser root must be a Program node");
    };
    let mut checker = Checker::new();
    let result = checker.check_program(program);
    (root, checker, result)
}

/// The match node's key: the first function body's first match
/// statement.
fn first_match_key(root: &Node) -> usize {
    let Node::Program(program) = root else {
        panic!("root must be a Program node");
    };
    for decl in &program.decls {
        if let Node::FuncDecl(f) = decl {
            for stmt in &f.body {
                if let Node::ExprStmt(inner) = stmt {
                    if matches!(&**inner, Node::MatchExpr(_)) {
                        return crate::types_util::node_key_of(inner);
                    }
                }
            }
        }
    }
    panic!("no match statement found");
}

/// Stage 5b checker-plans: `let y = v;` is an implicit let move
/// (check_let consumes a non-Copy source), so the Ok arm's payload
/// binding leaves Consumed and drops out of that arm's exit
/// cleanups — while the untouched Err payload stays listed.
/// Pre-fix this fails: the table is never recorded (None). The
/// emitter-side approximation cannot see this: the emitted body
/// contains no `^<v-alias>`, so it would release `v` after the
/// value already moved out through `y`.
#[test]
fn match_arm_exit_cleanups_realias_move_excluded() {
    let source = "fn f(r: Result<String, i32>) {\n    match r {\n        Ok(v) => {\n            let y = v;\n        },\n        Err(e) => {\n        },\n    };\n}\n";
    let (root, checker, result) = check_source_keep(source);
    assert!(result.is_ok(), "{result:?}");
    let key = first_match_key(&root);
    let arms = checker
        .match_arm_exit_cleanups
        .get(&key)
        .expect("check_match must record per-arm exit cleanups");
    assert_eq!(arms.len(), 2, "one list per arm, got {arms:?}");
    assert!(
        arms[0].is_empty(),
        "re-aliased payload must leave Consumed, got {:?}",
        arms[0]
    );
    assert_eq!(arms[1], vec!["e".to_string()]);
}

/// Control: a payload the arm body never touches stays listed.
#[test]
fn match_arm_exit_cleanups_untouched_payload_listed() {
    let source = "fn g(r: Result<String, i32>) {\n    match r {\n        Ok(v) => {\n        },\n        Err(e) => {\n        },\n    };\n}\n";
    let (root, checker, result) = check_source_keep(source);
    assert!(result.is_ok(), "{result:?}");
    let key = first_match_key(&root);
    let arms = checker
        .match_arm_exit_cleanups
        .get(&key)
        .expect("check_match must record per-arm exit cleanups");
    assert_eq!(arms.len(), 2, "one list per arm, got {arms:?}");
    assert_eq!(arms[0], vec!["v".to_string()]);
    assert_eq!(arms[1], vec!["e".to_string()]);
}

// -- stage 6 R2b: literal call arguments take the parameter's type --

/// The first call argument node in function `f`'s first return.
fn first_call_arg_key(root: &Node) -> usize {
    let Node::Program(program) = root else {
        panic!("root must be a Program node");
    };
    for decl in &program.decls {
        if let Node::FuncDecl(f) = decl {
            if f.name != "f" {
                continue;
            }
            for stmt in &f.body {
                if let Node::ReturnStmt(r) = stmt {
                    if let Some(Node::CallExpr(c)) = r.value.as_deref() {
                        return crate::types_util::node_key_of(&c.args[0]);
                    }
                }
            }
        }
    }
    panic!("no call return found in f");
}

/// R2b (stage 6): a bare-integer array literal passed to a
/// `[u8; 3]` parameter is accepted and the literal node's
/// recorded type becomes the parameter's array type (the
/// annotation recorded for the whole node, which the emitter
/// reads back). Pre-fix this was a TypeMismatch from the
/// width-strict array arm, and no recording happened.
#[test]
fn array_literal_arg_takes_param_type() {
    let source = "fn g(v: [u8; 3]) -> i32 { return 0; }\nfn f() -> i32 { return g([10, 20, 30]); }\n";
    let (root, checker, result) = check_source_keep(source);
    assert!(result.is_ok(), "{result:?}");
    let key = first_call_arg_key(&root);
    let recorded = checker
        .expr_types
        .get(&key)
        .expect("the literal argument's type must be recorded");
    let expected = sla_ast::types::Type::Array(sla_ast::types::ArrayType {
        elem: Box::new(sla_ast::types::Type::Primitive(
            sla_ast::types::Primitive::U8,
        )),
        len: 3,
    });
    assert_eq!(*recorded, expected);
}

/// Same discipline for a repeat literal argument.
#[test]
fn repeat_literal_arg_takes_param_type() {
    let source = "fn g(v: [i32; 3]) -> i32 { return 0; }\nfn f() -> i32 { return g([5; 3]); }\n";
    let (root, checker, result) = check_source_keep(source);
    assert!(result.is_ok(), "{result:?}");
    let key = first_call_arg_key(&root);
    let recorded = checker
        .expr_types
        .get(&key)
        .expect("the repeat argument's type must be recorded");
    let expected = sla_ast::types::Type::Array(sla_ast::types::ArrayType {
        elem: Box::new(sla_ast::types::Type::Primitive(
            sla_ast::types::Primitive::I32,
        )),
        len: 3,
    });
    assert_eq!(*recorded, expected);
}

/// Control: the width-strict rule still governs real values —
/// an `[i64; 3]` VARIABLE passed to a `[u8; 3]` parameter stays
/// a TypeMismatch (only literals, which have no width of their
/// own, take the parameter's element type).
#[test]
fn array_variable_arg_width_mismatch_still_rejected() {
    let source = "fn g(v: [u8; 3]) -> i32 { return 0; }\nfn f(a: [i64; 3]) -> i32 { return g(a); }\n";
    let result = check_source(source);
    let err = result.expect_err("width-mismatched variable must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch, "{err:?}");
}

/// LINE-FORIN-PROTOCOL: a concrete type offering the iterable
/// protocol (`iter_len(&self) -> i64`, `iter_at(&self, i64) ->
/// Elem`) is a legal for-in collection, and the loop variable
/// takes `iter_at`'s return type (Zig
/// `protocolIterableElementType`, type_checker.zig:4714). Before
/// the port the collection arm only accepted arrays and generic
/// user-defined types, so this was rejected with "for collection
/// is not iterable in this slice".
#[test]
fn for_in_protocol_impl_passes() {
    let source = "struct Item { x: i32 }\n\
struct Bag { len: i64, x0: i32, x1: i32 }\n\
impl Bag {\n\
    fn iter_len(&self) -> i64 { return self.len; }\n\
    fn iter_at(&self, index: i64) -> Item { return Item { x: self.x0 }; }\n\
}\n\
fn sum(bag: Bag) -> i32 {\n\
    let total: i32 = 0;\n\
    for item in bag { total = total + item.x; }\n\
    return total;\n\
}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Control: array for-in keeps passing (the element type is the
/// array's element type, as before the port).
#[test]
fn for_in_array_passes() {
    let source = "fn sum() -> i32 {\n\
    let xs = [10, 20, 12];\n\
    let total: i32 = 0;\n\
    for x in xs { total = total + x; }\n\
    return total;\n\
}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// A user-defined type WITHOUT the protocol methods is not
/// iterable: TypeMismatch with Zig's message and the value's
/// tag (type_checker.zig:8020).
#[test]
fn for_in_non_iterable_struct_rejected() {
    let source = "struct Plain { x: i32 }\n\
fn sum(p: Plain) -> i32 {\n\
    let total: i32 = 0;\n\
    for item in p { total = total + item.x; }\n\
    return total;\n\
}\n";
    let result = check_source(source);
    let err = result.expect_err("protocol-less struct must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch, "{err:?}");
    assert!(
        err.message.contains(
            "for iterable value must be array or implement Iterable: start tag=user_defined"
        ),
        "{err:?}"
    );
}

/// Zig parity: `Vec<T>` offers no `iter_len`/`iter_at`, so a
/// Vec for-in is rejected at check level (the old slice arm's
/// first-generic-argument heuristic accepted it here and left
/// the failure to codegen).
#[test]
fn for_in_vec_rejected_like_zig() {
    let source = "fn sum(v: Vec<i32>) -> i32 {\n\
    let total: i32 = 0;\n\
    for x in v { total = total + x; }\n\
    return total;\n\
}\n";
    let result = check_source(source);
    let err = result.expect_err("Vec for-in must fail at check level");
    assert_eq!(err.kind, ErrorKind::TypeMismatch, "{err:?}");
}
