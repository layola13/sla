//! R-6 regression tests, part 2: overload unification (extern /
//! imported candidate sets) and the header-less extern bypass.
//! Split from tests_r6.rs for the 700-line file cap; see that file
//! for the family-by-family corpus anchors.

use crate::checker::{Checker, ExternSig};
use crate::check_overload3::{extern_params_equal, extern_sig_from_parts};
use crate::diag::ErrorKind;
use sla_ast::decls::*;
use sla_ast::exprs::*;
use sla_ast::node::Node;
use sla_ast::stmts::*;
use sla_ast::types::*;

fn int_lit(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn expr_stmt(e: Node) -> Node {
    Node::ExprStmt(Box::new(e))
}

fn let_stmt(name: &str, ty: Option<Type>, value: Node) -> Node {
    Node::LetStmt(LetStmt {
        name: name.to_string(),
        ty: ty.map(Box::new),
        value: Box::new(value),
    })
}

fn ret_stmt(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt { value: Some(Box::new(value)) })
}

fn param(name: &str, ty: Type) -> Param {
    Param {
        name: name.to_string(),
        ty: Box::new(ty),
        is_borrow: false,
        is_move: false,
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
    })
}

fn call(name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: None,
        generics: Vec::new(),
        args,
        callee: None,
    })
}

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined { name: name.to_string(), generics })
}

fn string_ty() -> Type {
    ud("String", Vec::new())
}

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

// -- Overload unification (extern / imported sets) --------------------------
// Corpus anchors: test_unit_overload_extern_by_type /
// test_unit_ptr_out_param / test_unit_overload_imported.

fn extern_sig(spelling: &str, ret: &str) -> ExternSig {
    extern_sig_from_parts(
        &[("x".to_string(), spelling.to_string(), false, false)],
        ret,
        false,
    )
}

#[test]
fn r6_unify_extern_set_resolves_by_type() {
    let mut checker = Checker::new();
    checker
        .register_extern_contract("ext_pick", extern_sig("i32", "i32"))
        .unwrap();
    checker
        .register_extern_contract("ext_pick", extern_sig("f64", "f64"))
        .unwrap();
    let program = Program {
        decls: vec![func(
            "main",
            vec![],
            i32_ty(),
            vec![
                let_stmt("a", Some(i32_ty()), int_lit(1)),
                expr_stmt(call("ext_pick", vec![ident("a")])),
                ret_stmt(int_lit(0)),
            ],
        )],
    };
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn r6_unify_extern_same_params_diff_ret_is_redeclaration() {
    let mut checker = Checker::new();
    checker
        .register_extern_contract("ext_pick", extern_sig("i32", "i32"))
        .unwrap();
    let err = checker
        .register_extern_contract("ext_pick", extern_sig("i32", "f64"))
        .expect_err("same params, different return must be a Redeclaration");
    assert_eq!(err.kind, ErrorKind::Redeclaration);
}

#[test]
fn r6_unify_extern_identical_redeclaration_dedupes() {
    let mut checker = Checker::new();
    checker
        .register_extern_contract("ext_pick", extern_sig("i32", "i32"))
        .unwrap();
    let again = checker.register_extern_contract("ext_pick", extern_sig("i32", "i32"));
    assert!(again.is_ok(), "{again:?}");
    assert!(extern_params_equal(
        &extern_sig("i32", "i32"),
        &extern_sig("i32", "i32")
    ));
}

#[test]
fn r6_unify_imported_set_resolves() {
    let mut checker = Checker::new();
    let sig_i32 = crate::checker::FuncSig {
        params: vec![crate::checker::ParamSig {
            name: "x".to_string(),
            ty: i32_ty(),
            is_borrow: false,
            is_move: false,
        }],
        ret_ty: i32_ty(),
        is_pub: true,
        ret_fallible_sugar: false,
    };
    let sig_pair = crate::checker::FuncSig {
        params: vec![
            crate::checker::ParamSig {
                name: "x".to_string(),
                ty: i32_ty(),
                is_borrow: false,
                is_move: false,
            },
            crate::checker::ParamSig {
                name: "y".to_string(),
                ty: i32_ty(),
                is_borrow: false,
                is_move: false,
            },
        ],
        ret_ty: i32_ty(),
        is_pub: true,
        ret_fallible_sugar: false,
    };
    checker.register_imported_overload_member("imp_pick", sig_i32);
    checker.register_imported_overload_member("imp_pick", sig_pair);
    let program = Program {
        decls: vec![func(
            "main",
            vec![],
            i32_ty(),
            vec![
                let_stmt("a", Some(i32_ty()), int_lit(1)),
                expr_stmt(call("imp_pick", vec![ident("a")])),
                expr_stmt(call("imp_pick", vec![ident("a"), ident("a")])),
                ret_stmt(int_lit(0)),
            ],
        )],
    };
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn r6_headerless_extern_bypass_still_works() {
    // The two pre-existing unit-test contracts (params-less
    // ExternSig) keep their historical behavior through the new
    // unified dispatch: type bypass + move consumption.
    let mut checker = Checker::new();
    checker.register_extern_func(
        "c_write",
        ExternSig {
            move_params: vec![false],
            params: Vec::new(),
            is_variadic: false,
            ret_abi: "i32".to_string(),
            fallible: false,
        },
    );
    let program = Program {
        decls: vec![func(
            "f",
            vec![param("s", string_ty())],
            i32_ty(),
            vec![expr_stmt(call("c_write", vec![ident("s")]))],
        )],
    };
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "{result:?}");
}
