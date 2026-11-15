//! Regression (RUST-TEXT-EMITTER stage 5a resume, R3): `str_eq`
//! must compare with the byte-wise STR_EQ macro, not SLICE_EQ.
//!
//! SLICE_EQ_U64 compares u64 words at stride 8, over-reading
//! past the end of any byte string whose length is not a multiple
//! of 8 — so `str_eq(&s, "[a]")` failed while `len(s) == 3`
//! passed (test_unit_self_assign_move_out, panic 6716). Zig
//! emits STR_EQ here (codegen.zig:20085-20103); STR_EQ compares
//! byte by byte (sa_std/string.sa:5650).
//!
//! Pre-fix, `generate` emits `EXPAND SLICE_EQ`; post-fix it
//! emits `EXPAND STR_EQ`.

use sla_ast::{
    BorrowExpr, CallExpr, FuncDecl, Literal, Node, Param, Primitive, Program, ReturnStmt, Type,
};
use sla_lowering::MapCheckInfo;

fn string_ty() -> Type {
    Type::UserDefined(sla_ast::UserDefined {
        name: "String".to_string(),
        generics: Vec::new(),
    })
}

fn borrow_string_ty() -> Type {
    Type::Borrow(Box::new(string_ty()))
}

fn raw_ptr_ty() -> Type {
    Type::Primitive(Primitive::RawPtr)
}

fn bool_ty() -> Type {
    Type::Primitive(Primitive::Boolean)
}

#[test]
fn str_eq_uses_bytewise_str_eq_macro() {
    // fn f(s: String) -> bool { return str_eq(&s, "x"); }
    let call = Node::CallExpr(CallExpr {
        func_name: "str_eq".to_string(),
        associated_target: None,
        generics: Vec::new(),
        args: vec![
            Node::BorrowExpr(BorrowExpr {
                expr: Box::new(Node::Identifier("s".to_string())),
            }),
            Node::Literal(Literal::StringVal("x".to_string())),
        ],
        callee: None,
    });
    let program = Node::Program(Program {
        decls: vec![Node::FuncDecl(FuncDecl {
            name: "f".to_string(),
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics: Vec::new(),
            params: vec![Param {
                name: "s".to_string(),
                ty: Box::new(string_ty()),
                is_borrow: false,
                is_move: false,
                fallible_sugar: false,
                is_var: false,
            }],
            ret_ty: Box::new(bool_ty()),
            body: vec![Node::ReturnStmt(ReturnStmt {
                value: Some(Box::new(call)),
            })],
            is_inline: false,
            is_async: false,
            operator: None,
            is_arena: false,
            ret_fallible_sugar: false,
            ret_borrow_origin: None,
            is_variadic: false,
        })],
    });
    let mut check = MapCheckInfo::new();
    if let Node::Program(prog) = &program {
        if let Node::FuncDecl(f) = &prog.decls[0] {
            if let Node::ReturnStmt(r) = &f.body[0] {
                if let Some(inner) = &r.value {
                    if let Node::CallExpr(c) = &**inner {
                        check.add_expr_type(&c.args[0], borrow_string_ty());
                        check.add_expr_type(&c.args[1], raw_ptr_ty());
                        check.add_expr_type(inner, bool_ty());
                    }
                }
            }
        }
    }
    let out = sla_codegen_text::generate(&program, &check)
        .expect("str_eq call must emit on the text backend");
    assert!(
        out.contains("EXPAND STR_EQ "),
        "expected the byte-wise STR_EQ comparison, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND SLICE_EQ "),
        "the u64-word SLICE_EQ must not be used for string equality, got:\n{out}"
    );
}
