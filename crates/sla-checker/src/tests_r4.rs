//! R-4 regression tests: top-level const registration in the
//! checker (check_program Pass 2.5, mirroring Zig checkProgram
//! type_checker.zig:2271-2290).
//!
//! Corpus anchors: test_unit_top_level_numeric_const (direct
//! literal), test_unit_toplevel_const_alias (alias fixpoint),
//! test_unit_toplevel_const_binary_fold (binary fold),
//! test_unit_global_const_call_arg_cleanup (const as call arg).
//! On the pre-fix tree every one of these died in the checker on
//! the test-codegen form with `UndefinedVariable: <CONST>` while
//! `sla check` (decl-only bodies) accepted them.

use crate::checker::Checker;
use crate::diag::ErrorKind;
use sla_ast::decls::*;
use sla_ast::exprs::*;
use sla_ast::node::Node;
use sla_ast::stmts::*;
use sla_ast::types::*;

fn int_lit(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn bool_lit(v: bool) -> Node {
    Node::Literal(Literal::BoolVal(v))
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn bool_ty() -> Type {
    Type::Primitive(Primitive::Boolean)
}

fn const_stmt(name: &str, ty: Option<Type>, value: Node) -> Node {
    Node::ConstStmt(ConstStmt {
        name: name.to_string(),
        ty: ty.map(Box::new),
        value: Box::new(value),
    })
}

fn bin(op: BinaryOp, l: Node, r: Node) -> Node {
    Node::BinaryExpr(BinaryExpr {
        op,
        left: Box::new(l),
        right: Box::new(r),
    })
}

fn param(name: &str, ty: Type) -> Param {
    Param {
        name: name.to_string(),
        ty: Box::new(ty),
        is_borrow: false,
        is_move: false,
        fallible_sugar: false,
        is_var: false,
    }
}

fn func(name: &str, params: Vec<Param>, ret: Type, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params,
        ret_ty: Box::new(ret),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
        is_variadic: false,
    })
}

fn ret_stmt(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(value)),
    })
}

fn check(decls: Vec<Node>) -> Result<(), crate::diag::Diagnostic> {
    let mut checker = Checker::new();
    checker.check_program(&Program { decls })
}

/// A literal top-level const is visible inside a function body
/// (corpus: test_unit_top_level_numeric_const).
#[test]
fn top_level_const_visible_in_fn_body() {
    let result = check(vec![
        const_stmt("ANSWER", Some(i32_ty()), int_lit(41)),
        func(
            "get",
            vec![],
            i32_ty(),
            vec![ret_stmt(bin(BinaryOp::Add, ident("ANSWER"), int_lit(1)))],
        ),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

/// A bool const and an alias chain resolve through the global
/// scope (corpus: test_unit_toplevel_const_alias).
#[test]
fn top_level_const_alias_chain() {
    let result = check(vec![
        const_stmt("BASE", Some(i32_ty()), int_lit(7)),
        const_stmt("ALIAS", Some(i32_ty()), ident("BASE")),
        const_stmt("FLAG", Some(bool_ty()), bool_lit(true)),
        func(
            "get",
            vec![],
            i32_ty(),
            vec![ret_stmt(ident("ALIAS"))],
        ),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

/// A binary-fold const value checks in the global scope
/// (corpus: test_unit_toplevel_const_binary_fold).
#[test]
fn top_level_const_binary_fold_value() {
    let result = check(vec![
        const_stmt("A", Some(i32_ty()), int_lit(20)),
        const_stmt("B", Some(i32_ty()), int_lit(22)),
        const_stmt(
            "COMBINED",
            Some(i32_ty()),
            bin(BinaryOp::Add, ident("A"), ident("B")),
        ),
        func(
            "get",
            vec![],
            i32_ty(),
            vec![ret_stmt(ident("COMBINED"))],
        ),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

/// A const whose declared type rejects the value type is a
/// TypeMismatch naming the const (Zig checkProgram const arm).
#[test]
fn top_level_const_type_mismatch_rejected() {
    let result = check(vec![const_stmt("BAD", Some(bool_ty()), int_lit(1))]);
    let err = result.expect_err("mismatched const must be rejected");
    assert_eq!(err.kind, ErrorKind::TypeMismatch, "{err:?}");
    assert!(
        err.message.contains("BAD"),
        "diagnostic must name the const: {err:?}"
    );
}

/// A const whose value references an undefined name stays an
/// UndefinedVariable rejection (the value is really checked, not
/// blindly registered).
#[test]
fn top_level_const_undefined_value_rejected() {
    let result = check(vec![const_stmt(
        "BROKEN",
        Some(i32_ty()),
        ident("NOPE"),
    )]);
    let err = result.expect_err("undefined const value must be rejected");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable, "{err:?}");
}
