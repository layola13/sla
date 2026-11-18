//! F-B7 regression test (LINE-STACKESCAPE-FIX).
//!
//! `__slice_view(ptr, len)` is the Rust emitter's intrinsic for the
//! generated sla_std wrappers' `&[u8]` return form. The F-B1 arm
//! materialised the view cell with `stack_alloc Slice_SIZE`, so a
//! wrapper returning it was rejected by the host verifier with
//! `error[StackEscape]: stack allocation cannot be returned` —
//! the cell escapes its frame. Zig emits `alloc Slice_SIZE` for
//! the same production (codegen.zig `genSliceViewCall`).
//!
//! On the base emitter this test fails: the emission contains
//! `stack_alloc Slice_SIZE` and no `alloc Slice_SIZE`.

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

/// fn f(data: *u8, len: u64) -> &[u8] { return __slice_view(data, len); }
/// Mirrors the generated sla_std wrapper shape (e.g.
/// `sla_fs_read_buffer_data` in test_unit_borrow_return_views).
#[test]
fn slice_view_emits_heap_alloc_not_stack_alloc() {
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            vec![
                param("data", Type::Primitive(Primitive::RawPtr)),
                param("len", u64_ty()),
            ],
            slice_u8_borrow_ty(),
            vec![ret(call(
                "__slice_view",
                vec![ident("data"), ident("len")],
            ))],
        )],
    });
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&program, &check)
        .expect("__slice_view must emit on the text backend");
    assert!(
        out.contains("alloc Slice_SIZE"),
        "expected a heap-allocated view cell, got:\n{out}"
    );
    assert!(
        !out.contains("stack_alloc Slice_SIZE"),
        "view cell must not be stack-allocated (StackEscape), got:\n{out}"
    );
}
