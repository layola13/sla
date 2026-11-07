//! Regression tests for the std leaf cascade port
//! (check_std_ctor.rs / check_std_leaf.rs / check_std_leaf2.rs),
//! phase-6 census root class R-2 (and its same-root sibling R-1).
//!
//! Baseline behavior (cc52e91): every program below failed the
//! checker with `Undefined call: new` / `Undefined call: push` /
//! `Undefined call: chain_point_sum`-family errors in whichever
//! bodies the frontend parsed — plain `fn` bodies under
//! `sla check`, `@test` bodies under `sla test`. The minimal
//! repro is the single-@test file: `sla check` passed it
//! (test bodies are skipped on that path) while `sla test`
//! failed it on both backends.

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

fn check_source_with_test_bodies(source: &str) -> Result<(), Diagnostic> {
    let options = sla_parser::ParserOptions {
        parse_test_bodies: true,
        ..Default::default()
    };
    let mut parser = sla_parser::Parser::with_options(source, options);
    let root = parser.parse_program().expect("source must parse");
    let Node::Program(program) = &root else {
        panic!("parser root must be a Program node");
    };
    let mut checker = Checker::new();
    checker.check_program(program)
}

/// The R-2 minimal repro, verbatim shape: a single @test file
/// whose body uses the std Vec surface. With test bodies parsed
/// (the TestCodegen front-end), the checker must accept it.
#[test]
fn r2_single_test_file_vec_surface_passes() {
    let source = "@test \"vec basics\"() {\n    let v = Vec::new();\n    v.push(1);\n    if (len(v) as u64) != 1 { panic(1); };\n}\n";
    let result = check_source_with_test_bodies(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Same surface in a plain fn body (the R-1 manifestation of the
/// same root): `fn main` + a helper, no @test involved.
#[test]
fn r1_vec_surface_in_plain_fn_passes() {
    let source = "fn helper() -> i32 {\n    let v = Vec::new();\n    v.push(41);\n    return (len(v) as i32);\n}\nfn main() -> i32 {\n    return helper();\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Container constructors + map/set method leaves.
#[test]
fn r2_hashmap_insert_get_passes() {
    let source = "fn main() -> i32 {\n    let m = HashMap::new();\n    m.insert(\"a\", 40);\n    m.insert(\"b\", 2);\n    let got = m.get(\"a\");\n    if got.is_none() { return -1; };\n    return 0;\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Option leaf arms: query + unwrap_or through a VecDeque
/// pop_front (which yields the Option the arms dispatch on).
#[test]
fn r2_option_result_leaves_pass() {
    let source = "fn main() -> i32 {\n    let q = VecDeque::new();\n    q.push_back(7);\n    let first = q.pop_front();\n    if first.is_none() { return -1; };\n    return first.unwrap_or(0);\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Vec name-keyed arms: push / pop / remove typing.
#[test]
fn r2_vec_pop_remove_pass() {
    let source = "fn main() -> i32 {\n    let v = Vec::new();\n    v.push(1);\n    v.push(2);\n    let last = v.pop();\n    let first = v.remove(0);\n    return first + last.unwrap_or(0);\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Iterator combinators over a Vec: iter / map / fold arms with
/// annotated closure parameters (the SLA source form).
#[test]
fn r2_iterator_combinators_pass() {
    let source = "fn main() -> i32 {\n    let v = Vec::new();\n    v.push(1);\n    v.push(2);\n    let doubled = v.iter().map(|x: i32| x * 2);\n    return (len(doubled) as i32);\n}\n";
    let result = check_source(source);
    assert!(result.is_ok(), "{result:?}");
}

/// Over-acceptance guard: a push of the wrong element type is a
/// TypeMismatch, not a pass (the leaf arms check arguments).
#[test]
fn r2_push_wrong_element_type_rejected() {
    let source = "fn main() -> i32 {\n    let v = Vec::new();\n    v.push(1);\n    v.push(\"nope\");\n    return 0;\n}\n";
    let result = check_source(source);
    assert!(result.is_err(), "{result:?}");
}

/// Over-acceptance guard: constructor arity is enforced.
#[test]
fn r2_vec_new_with_argument_rejected() {
    let source = "fn main() -> i32 {\n    let v = Vec::new(3);\n    return (len(v) as i32);\n}\n";
    let result = check_source(source);
    let err = result.expect_err("Vec::new(3) must be rejected");
    assert_eq!(err.kind, ErrorKind::InvalidArgsCount);
}

/// The Infer-refinement discipline: an unannotated HashMap
/// binding takes its key/value types from the first insert, and
/// a later insert of a different value type is rejected.
#[test]
fn r2_hashmap_infer_refinement_binds() {
    let source = "fn main() -> i32 {\n    let m = HashMap::new();\n    m.insert(\"a\", 1);\n    m.insert(\"b\", \"nope\");\n    return 0;\n}\n";
    let result = check_source(source);
    assert!(result.is_err(), "{result:?}");
}

