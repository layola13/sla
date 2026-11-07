//! Phase-3 tests, part A (CHK3 items 3 and 2): plain-`if`
//! condition acceptance and match-guard visibility / consumption.
//! All expectations are pinned to venue truth: real plugin at
//! head 6002b386, `sa sla check` on the probe sources below
//! (venue 3, 2026-11-07; probes kept in the CHK3 working notes,
//! results quoted per test). Tests run the real pipeline shape
//! (source -> parse -> check), per the INT-1 lesson.

use crate::checker::Checker;
use crate::diag::{Diagnostic, ErrorKind};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

fn check_source(source: &str) -> Result<(), Diagnostic> {
    let root = sla_parser::parse_program(source).expect("source must parse");
    let Node::Program(program) = &root else {
        panic!("parser root must be a Program node");
    };
    let mut checker = Checker::new();
    checker.check_program(program)
}

fn check_ast(program: sla_ast::decls::Program) -> Result<(), Diagnostic> {
    let mut checker = Checker::new();
    checker.check_program(&program)
}

// -- item 3: plain-if condition ------------------------------------
// Zig plain-if arm (venue type_checker.zig :10150): infer passes
// unconditionally; a primitive passes only as boolean or legacy
// `integer`; everything else is TypeMismatch with no setError.
// Venue probes: bool rc=0; comparison rc=0 (comparisons produce
// legacy integer); int literal rc=1; i32 var rc=1; `integer`-
// annotated var rc=1 (the source annotation does NOT produce the
// legacy integer checker type); `int`-annotated var rc=1; f64
// literal rc=1; String var rc=1.

#[test]
fn if_bool_condition_passes() {
    let src = "fn f(b: bool) {\n    if b {\n        let x = 1;\n    }\n}\n";
    assert!(check_source(src).is_ok());
}

#[test]
fn if_comparison_condition_passes() {
    // Comparisons produce legacy `integer` in Zig; in this crate
    // they type as boolean — either arm of the acceptance set
    // covers the shape, and the venue verdict is rc=0.
    let src = "fn f(v: i32) {\n    if v > 0 {\n        let x = 1;\n    }\n}\n";
    assert!(check_source(src).is_ok());
}

#[test]
fn if_int_literal_condition_rejected() {
    // Venue rc=1 TypeMismatch: an int literal types as a SIZED
    // integer (i64 here), not the legacy `integer`.
    let src = "fn f() {\n    if 1 {\n        let x = 1;\n    }\n}\n";
    let err = check_source(src).expect_err("int literal cond must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn if_sized_int_condition_rejected() {
    let src = "fn f(v: i32) {\n    if v {\n        let x = 1;\n    }\n}\n";
    let err = check_source(src).expect_err("i32 cond must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn if_float_condition_rejected() {
    let src = "fn f() {\n    if 1.5 {\n        let x = 1;\n    }\n}\n";
    let err = check_source(src).expect_err("f64 cond must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn if_string_condition_rejected() {
    let src = "fn f(s: String) {\n    if s {\n        let x = 1;\n    }\n}\n";
    let err = check_source(src).expect_err("String cond must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn if_legacy_integer_condition_passes() {
    // AST-level: a binding whose checker type IS the legacy
    // `integer` primitive (produced in Zig by comparisons and
    // macro parameters, not by source annotations) is accepted.
    use sla_ast::decls::*;
    use sla_ast::exprs::*;
    use sla_ast::stmts::*;
    let func = Node::FuncDecl(FuncDecl {
        name: "f".to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: vec![],
        params: vec![Param {
            name: "v".to_string(),
            ty: Box::new(Type::Primitive(Primitive::Integer)),
            is_borrow: false,
            is_move: false,
        }],
        ret_ty: Box::new(Type::Primitive(Primitive::VoidType)),
        body: vec![Node::ExprStmt(Box::new(Node::IfExpr(IfExpr {
            cond: Box::new(Node::Identifier("v".to_string())),
            let_chain: None,
            then_block: vec![Node::LetStmt(LetStmt {
                name: "x".to_string(),
                ty: None,
                value: Box::new(Node::Literal(Literal::IntVal(1))),
            })],
            else_block: None,
        })))],
        is_inline: false,
        is_async: false,
        operator: None,
    });
    let result = check_ast(Program { decls: vec![func] });
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn if_infer_condition_passes() {
    // The macro-parameter case in Zig's comment: an `infer`-typed
    // condition passes unconditionally.
    use sla_ast::decls::*;
    use sla_ast::exprs::*;
    use sla_ast::stmts::*;
    let func = Node::FuncDecl(FuncDecl {
        name: "f".to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: vec![],
        params: vec![Param {
            name: "v".to_string(),
            ty: Box::new(Type::Infer),
            is_borrow: false,
            is_move: false,
        }],
        ret_ty: Box::new(Type::Primitive(Primitive::VoidType)),
        body: vec![Node::ExprStmt(Box::new(Node::IfExpr(IfExpr {
            cond: Box::new(Node::Identifier("v".to_string())),
            let_chain: None,
            then_block: vec![Node::LetStmt(LetStmt {
                name: "x".to_string(),
                ty: None,
                value: Box::new(Node::Literal(Literal::IntVal(1))),
            })],
            else_block: None,
        })))],
        is_inline: false,
        is_async: false,
        operator: None,
    });
    let result = check_ast(Program { decls: vec![func] });
    assert!(result.is_ok(), "{result:?}");
}

// -- item 2: match guards ------------------------------------------
// Zig (venue match arms): pattern bindings are defined in a fresh
// pattern scope BEFORE the guard; guard and case body are checked
// against that same scope chain, so guard consumption of both the
// pattern binding and outer bindings persists into the body, and
// the case's live end state (saved from the OUTER scope after
// guard + body) feeds the multi-branch merge — there is NO
// rollback for the guard-false path. Guard type: primitive
// boolean / legacy integer only (sized ints and int literals
// rejected, venue rc=1 TypeMismatch at match_expr).

#[test]
fn guard_sees_pattern_binding() {
    let src = "fn f(o: Option<i32>) -> i32 {\n    return match o {\n        Some(v) if v > 100 => { v },\n        Some(v) => { v },\n        None => { 0 },\n    };\n}\n";
    assert!(check_source(src).is_ok());
}

#[test]
fn guard_move_of_pattern_binding_poisons_body() {
    // Venue rc=1 UseAfterMove (`v`): the guard's `^v` consumes the
    // pattern binding in the shared pattern scope.
    let src = "fn f(o: Option<i32>) -> i32 {\n    return match o {\n        Some(v) if ^v > 100 => { v },\n        Some(v) => { v },\n        None => { 0 },\n    };\n}\n";
    let err = check_source(src).expect_err("guard move must poison body");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn guard_move_alone_passes() {
    // Venue rc=0: consuming the pattern binding in the guard is
    // fine when the body does not use it again.
    let src = "fn f(o: Option<i32>) -> i32 {\n    return match o {\n        Some(v) if ^v > 100 => { 1 },\n        Some(v) => { v },\n        None => { 0 },\n    };\n}\n";
    assert!(check_source(src).is_ok());
}

#[test]
fn guard_consumption_of_outer_binding_persists_into_body() {
    // Venue rc=1 UseAfterMove (`o2`): the guard's try consumption
    // of the outer `o2` is visible to the case body.
    let src = "fn f(o: Option<i32>, o2: Option<i32>) -> i32 {\n    return match o {\n        Some(v) if (o2?) == 7 => { (o2?) },\n        Some(v) => { v },\n        None => { 0 },\n    };\n}\n";
    let err = check_source(src).expect_err("guard consumption must persist");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn guard_consumption_merges_without_rollback() {
    // Venue rc=1 UseAfterMove (`o2`) AFTER the match: the guard's
    // consumption enters the case's live state and the merge —
    // the checker does not roll back for a guard-false path.
    let src = "fn f(o: Option<i32>, o2: Option<i32>) -> i32 {\n    let r = match o {\n        Some(v) if (o2?) == 7 => { v },\n        Some(v) => { v },\n        None => { 0 },\n    };\n    let z = (o2?);\n    return r + z;\n}\n";
    let err = check_source(src).expect_err("merged guard consumption must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn guard_sized_int_rejected() {
    let src = "fn f(o: Option<i32>) -> i32 {\n    return match o {\n        Some(v) if v => { v },\n        Some(v) => { v },\n        None => { 0 },\n    };\n}\n";
    let err = check_source(src).expect_err("i32 guard must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn guard_int_literal_rejected() {
    // Venue rc=1: a literal guard types as a sized integer.
    let src = "fn f(o: Option<i32>) -> i32 {\n    return match o {\n        Some(v) if 1 => { v },\n        Some(v) => { v },\n        None => { 0 },\n    };\n}\n";
    let err = check_source(src).expect_err("literal guard must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn guard_cannot_see_other_case_binding() {
    // Venue rc=1 UndefinedVariable (`v`): bindings are per-case.
    let src = "fn f(o: Option<i32>) -> i32 {\n    return match o {\n        Some(v) => { v },\n        None if v > 0 => { 0 },\n        None => { 0 },\n    };\n}\n";
    let err = check_source(src).expect_err("cross-case binding must fail");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
}

#[test]
fn pattern_binding_does_not_escape_match() {
    // Venue rc=1 UndefinedVariable (`v`) after the match.
    let src = "fn f(o: Option<i32>) -> i32 {\n    let r = match o {\n        Some(v) => { v },\n        None => { 0 },\n    };\n    return r + v;\n}\n";
    let err = check_source(src).expect_err("escaped binding must fail");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
}

#[test]
fn enum_guard_move_of_binding_poisons_body() {
    // Venue rc=1 UseAfterMove (`x`) on the plain-enum path too.
    let src = "enum Pick {\n    Have { x: i32 },\n    Lack,\n}\nfn f(e: Pick) -> i32 {\n    return match e {\n        Pick::Have { x } if ^x > 3 => { x },\n        Pick::Have { x } => { x },\n        Pick::Lack => { 0 },\n    };\n}\n";
    let err = check_source(src).expect_err("enum guard move must poison body");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}
