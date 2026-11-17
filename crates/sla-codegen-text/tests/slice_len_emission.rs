//! F-B6 regression test (LINE-LEN follow-up).
//!
//! `view.len()` on a `&[u8]` borrow classified to ("Slice", [u8])
//! but the free-`len` table had no Slice arm, so the call fell to
//! the bare-name EXPAND catch-all, synthesising `EXPAND len tmp, view`
//! for a macro that exists in no std tree (UndefinedMacro at
//! assembly — test_unit_borrow_return_views @sla__view_eq, line 843).
//! Zig emits `EXPAND SLICE_GET_LEN len, slice`
//! (sa_std/core/slice.sa; demos/c/*/main.sa).
//!
//! On the base emitter this test fails with the bare-name EXPAND.

use sla_ast::*;
use sla_lowering::MapCheckInfo;

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

fn u8_ty() -> Type {
    Type::Primitive(Primitive::U8)
}

fn u64_ty() -> Type {
    Type::Primitive(Primitive::U64)
}

fn slice_u8_borrow_ty() -> Type {
    Type::Borrow(Box::new(ud("Slice", vec![u8_ty()])))
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

fn func(name: &str, params: Vec<Param>, ret_ty: Type, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params,
        ret_ty: Box::new(ret_ty),
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

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn call(func_name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: func_name.to_string(),
        associated_target: None,
        generics: Vec::new(),
        args,
        callee: None,
    })
}

fn ret(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(value)),
    })
}

/// fn f(view: &[u8]) -> u64 { return view.len(); }
/// The receiver type comes from the parameter prologue's binding
/// type (no checker expr-type fact registered), as in the real
/// corpus path.
#[test]
fn slice_borrow_len_emits_slice_get_len() {
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("view", slice_u8_borrow_ty())],
            u64_ty(),
            vec![ret(call("len", vec![ident("view")]))],
        )],
    });
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&program, &check)
        .expect("len on a slice borrow must emit on the text backend");
    assert!(
        out.contains("EXPAND SLICE_GET_LEN "),
        "expected SLICE_GET_LEN for len on &[u8], got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND len "),
        "must not emit the bare-name EXPAND catch-all, got:\n{out}"
    );
}
