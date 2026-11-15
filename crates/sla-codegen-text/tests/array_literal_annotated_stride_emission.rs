//! Regression (RUST-TEXT-EMITTER stage 5a resume, R2b): an
//! annotated array let of bare integer elements must lay out at
//! the ANNOTATION's stride, not the elements' default type.
//!
//! `let suffix: [u8; 13] = [46, 105, ...]` in
//! dns_reverse_name_v4 (and `let expect: [u8; 10] = [...]` in
//! dns_resolv_conf_server) was stored one byte per 8-byte slot
//! (`store suffix+8, ... as i64`) because the checker's literal
//! type is [i64; 13] — untyped ints default to i64 — while the
//! index read strides by 1 and loads u8, so every element at
//! index >= 1 read back 0 (Zig genArrayLiteralInto :16365-16371
//! takes the element type from the annotation recorded for the
//! whole node).
//!
//! Pre-fix, `generate` emits `a = alloc 16` with
//! `store a+8, ... as i64`; post-fix it emits `a = alloc 2` with
//! `store a+1, ... as u8`.

use sla_ast::{
    ArrayLiteral, FuncDecl, LetStmt, Literal, Node, Primitive, Program, ReturnStmt, Type,
};
use sla_lowering::MapCheckInfo;

fn u8_ty() -> Type {
    Type::Primitive(Primitive::U8)
}

fn array_u8_ty(len: usize) -> Type {
    Type::Array(sla_ast::ArrayType {
        elem: Box::new(u8_ty()),
        len,
    })
}

fn void_ty() -> Type {
    Type::Primitive(Primitive::VoidType)
}

#[test]
fn annotated_array_let_of_bare_ints_lays_out_at_u8_stride() {
    // fn f() { let a: [u8; 2] = [13, 10]; return; }
    let program = Node::Program(Program {
        decls: vec![Node::FuncDecl(FuncDecl {
            name: "f".to_string(),
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics: Vec::new(),
            params: Vec::new(),
            ret_ty: Box::new(void_ty()),
            body: vec![
                Node::LetStmt(LetStmt {
                    name: "a".to_string(),
                    ty: Some(Box::new(array_u8_ty(2))),
                    value: Box::new(Node::ArrayLiteral(ArrayLiteral {
                        elements: vec![
                            Node::Literal(Literal::IntVal(13)),
                            Node::Literal(Literal::IntVal(10)),
                        ],
                    })),
                }),
                Node::ReturnStmt(ReturnStmt { value: None }),
            ],
            is_inline: false,
            is_async: false,
            operator: None,
            is_arena: false,
            ret_fallible_sugar: false,
            ret_borrow_origin: None,
            is_variadic: false,
        })],
    });
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&program, &check)
        .expect("annotated array let must emit on the text backend");
    assert!(
        out.contains("a = alloc 2"),
        "expected a 2-byte allocation for [u8; 2], got:\n{out}"
    );
    assert!(
        out.contains("store a+1, ") && out.contains(" as u8"),
        "expected the second element stored at stride 1 as u8, got:\n{out}"
    );
    assert!(
        !out.contains("store a+8,"),
        "no element may be stored at i64 stride 8, got:\n{out}"
    );
}
