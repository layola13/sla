//! Regression (RUST-TEXT-EMITTER stage 6, R2b remainder): an
//! array / repeat literal must take its element layout from the
//! CONSUMING CONTEXT everywhere, not only in the two positions
//! stage 5a covered (annotated plain lets, struct field values).
//!
//! The checker records bare integer elements as i64, so a literal
//! consumed as `[u8; N]` / `[i32; N]` was laid out at i64 stride
//! in return position, as a call argument, nested inside another
//! literal, and as a repeat in an annotated let or a struct
//! field — and a repeat with no recorded node type failed with
//! MissingType. Zig takes the element type from the annotation
//! recorded for the whole node (codegen.zig:16365-16371).
//!
//! Every test here fails on the pre-fix tree (wrong stride in
//! the emitted text, or a MissingType error) and passes with
//! the stage-6 fix.

use sla_ast::{
    ArrayLiteral, CallExpr, Field, FuncDecl, LetStmt, Literal, Node, Param, Primitive, Program,
    RepeatArrayLiteral, ReturnStmt, StructDecl, StructLiteral, StructLiteralField, Type,
};
use sla_lowering::MapCheckInfo;

fn prim(p: Primitive) -> Type {
    Type::Primitive(p)
}

fn arr_ty(elem: Type, len: usize) -> Type {
    Type::Array(sla_ast::ArrayType {
        elem: Box::new(elem),
        len,
    })
}

fn void_ty() -> Type {
    prim(Primitive::VoidType)
}

fn int_lit(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
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

/// The array-literal node inside `return [..];` of decl 0.
fn return_value_node(program: &Node) -> &Node {
    let Node::Program(p) = program else {
        panic!("root must be a Program node");
    };
    let Node::FuncDecl(f) = &p.decls[0] else {
        panic!("decl 0 must be a function");
    };
    let Node::ReturnStmt(r) = &f.body[0] else {
        panic!("body 0 must be a return statement");
    };
    r.value.as_deref().expect("return must carry a value")
}

#[test]
fn repeat_literal_annotated_let_u8_fill() {
    // fn f() { let a: [u8; 4] = [7; 4]; return; }
    // Pre-fix: the let's repeat arm never consulted the
    // annotation and the node has no recorded type in this
    // harness, so emission failed with MissingType.
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            Vec::new(),
            void_ty(),
            vec![
                Node::LetStmt(LetStmt {
                    name: "a".to_string(),
                    ty: Some(Box::new(arr_ty(prim(Primitive::U8), 4))),
                    value: Box::new(Node::RepeatArrayLiteral(RepeatArrayLiteral {
                        value: Box::new(int_lit(7)),
                        len: 4,
                    })),
                }),
                Node::ReturnStmt(ReturnStmt { value: None }),
            ],
        )],
    });
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&program, &check)
        .expect("annotated repeat let must emit on the text backend");
    assert!(
        out.contains("a = alloc 4"),
        "expected a 4-byte allocation for [u8; 4], got:\n{out}"
    );
    assert!(
        out.contains("EXPAND SLA_ARRAY_FILL_U8 a,"),
        "expected the u8 fill macro against the binding, got:\n{out}"
    );
}

#[test]
fn array_literal_return_uses_return_type_stride() {
    // fn f() -> [u8; 3] { return [10, 20, 30]; }
    // The harness records the literal's own type as [i64; 3],
    // exactly as the checker does for bare integer elements.
    // Pre-fix the return emitted alloc 24 with i64 stores.
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            Vec::new(),
            arr_ty(prim(Primitive::U8), 3),
            vec![Node::ReturnStmt(ReturnStmt {
                value: Some(Box::new(Node::ArrayLiteral(ArrayLiteral {
                    elements: vec![int_lit(10), int_lit(20), int_lit(30)],
                }))),
            })],
        )],
    });
    let mut check = MapCheckInfo::new();
    check.add_expr_type(
        return_value_node(&program),
        arr_ty(prim(Primitive::I64), 3),
    );
    let out = sla_codegen_text::generate(&program, &check)
        .expect("array literal return must emit on the text backend");
    assert!(
        out.contains("alloc 3"),
        "expected a 3-byte allocation for [u8; 3], got:\n{out}"
    );
    assert!(
        !out.contains("alloc 24"),
        "the literal must not be laid out at i64 stride, got:\n{out}"
    );
    assert!(
        out.contains("+2, ") && out.contains(" as u8"),
        "expected the third element stored at stride 1 as u8, got:\n{out}"
    );
}

#[test]
fn repeat_literal_return_uses_return_type_stride() {
    // fn f() -> [i32; 3] { return [5; 3]; } with the literal's
    // own recorded type [i64; 3]. Pre-fix: alloc 24, i64 stores.
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            Vec::new(),
            arr_ty(prim(Primitive::I32), 3),
            vec![Node::ReturnStmt(ReturnStmt {
                value: Some(Box::new(Node::RepeatArrayLiteral(RepeatArrayLiteral {
                    value: Box::new(int_lit(5)),
                    len: 3,
                }))),
            })],
        )],
    });
    let mut check = MapCheckInfo::new();
    check.add_expr_type(
        return_value_node(&program),
        arr_ty(prim(Primitive::I64), 3),
    );
    let out = sla_codegen_text::generate(&program, &check)
        .expect("repeat literal return must emit on the text backend");
    assert!(
        out.contains("alloc 12"),
        "expected a 12-byte allocation for [i32; 3], got:\n{out}"
    );
    assert!(
        !out.contains("alloc 24"),
        "the repeat must not be laid out at i64 stride, got:\n{out}"
    );
    assert!(
        out.contains(" as i32"),
        "expected the fill stored as i32, got:\n{out}"
    );
}

#[test]
fn nested_array_literal_annotated_let() {
    // fn f() { let m: [[u8; 2]; 2] = [[1, 2], [3, 4]]; return; }
    // Pre-fix: the outer typed path supplied [u8; 2] as the
    // element type but each inner literal went through gen_expr
    // with no recorded type and failed with MissingType.
    let inner = |a: i64, b: i64| {
        Node::ArrayLiteral(ArrayLiteral {
            elements: vec![int_lit(a), int_lit(b)],
        })
    };
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            Vec::new(),
            void_ty(),
            vec![
                Node::LetStmt(LetStmt {
                    name: "m".to_string(),
                    ty: Some(Box::new(arr_ty(arr_ty(prim(Primitive::U8), 2), 2))),
                    value: Box::new(Node::ArrayLiteral(ArrayLiteral {
                        elements: vec![inner(1, 2), inner(3, 4)],
                    })),
                }),
                Node::ReturnStmt(ReturnStmt { value: None }),
            ],
        )],
    });
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&program, &check)
        .expect("nested annotated literal must emit on the text backend");
    assert_eq!(
        out.matches("alloc 2").count(),
        2,
        "expected each inner [u8; 2] literal allocated at 2 bytes, got:\n{out}"
    );
    assert!(
        !out.contains(" as i64"),
        "no element may be stored at i64 width, got:\n{out}"
    );
}

#[test]
fn repeat_literal_struct_field_uses_field_type() {
    // struct Buf { data: [u8; 3], len: i64 }
    // fn f() { let b = Buf { data: [7; 3], len: 3 }; return; }
    // Pre-fix: the field path handled plain literals and tuples
    // only; the repeat fell through to gen_expr and failed with
    // MissingType (no recorded node type in field position).
    let buf_ty = Type::UserDefined(sla_ast::UserDefined {
        name: "Buf".to_string(),
        generics: Vec::new(),
    });
    let lit = Node::StructLiteral(StructLiteral {
        ty: Box::new(buf_ty.clone()),
        fields: vec![
            StructLiteralField {
                name: "data".to_string(),
                value: Box::new(Node::RepeatArrayLiteral(RepeatArrayLiteral {
                    value: Box::new(int_lit(7)),
                    len: 3,
                })),
            },
            StructLiteralField {
                name: "len".to_string(),
                value: Box::new(int_lit(3)),
            },
        ],
        update_expr: None,
    });
    let program = Node::Program(Program {
        decls: vec![func(
            "f",
            Vec::new(),
            void_ty(),
            vec![
                Node::LetStmt(LetStmt {
                    name: "b".to_string(),
                    ty: None,
                    value: Box::new(lit),
                }),
                Node::ReturnStmt(ReturnStmt { value: None }),
            ],
        )],
    });
    let mut check = MapCheckInfo::new();
    check.add_struct(StructDecl {
        name: "Buf".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
        fields: vec![
            Field {
                name: "data".to_string(),
                ty: Box::new(arr_ty(prim(Primitive::U8), 3)),
            },
            Field {
                name: "len".to_string(),
                ty: Box::new(prim(Primitive::I64)),
            },
        ],
        is_union: false,
        is_opaque: false,
    });
    // The let's value type (the struct literal's own type).
    let Node::Program(p) = &program else {
        panic!("root must be a Program node");
    };
    let Node::FuncDecl(f) = &p.decls[0] else {
        panic!("decl 0 must be a function");
    };
    let Node::LetStmt(l) = &f.body[0] else {
        panic!("body 0 must be a let statement");
    };
    check.add_expr_type(&l.value, buf_ty);
    let out = sla_codegen_text::generate(&program, &check)
        .expect("struct literal with repeat field must emit on the text backend");
    assert!(
        out.contains("EXPAND SLA_ARRAY_FILL_U8"),
        "expected the u8 fill macro for the field's repeat literal, got:\n{out}"
    );
}

#[test]
fn array_literal_call_arg_uses_param_stride() {
    // fn g(v: [u8; 3]) { return; }
    // fn f() { g([10, 20, 30]); return; }
    // The harness records the argument literal's own type as
    // [i64; 3] (the pre-fix checker recording). Pre-fix the
    // argument was materialised at i64 stride (alloc 24).
    let call = Node::CallExpr(CallExpr {
        func_name: "g".to_string(),
        associated_target: None,
        generics: Vec::new(),
        args: vec![Node::ArrayLiteral(ArrayLiteral {
            elements: vec![int_lit(10), int_lit(20), int_lit(30)],
        })],
        callee: None,
    });
    let program = Node::Program(Program {
        decls: vec![
            func(
                "g",
                vec![param("v", arr_ty(prim(Primitive::U8), 3))],
                void_ty(),
                vec![Node::ReturnStmt(ReturnStmt { value: None })],
            ),
            func(
                "f",
                Vec::new(),
                void_ty(),
                vec![
                    Node::ExprStmt(Box::new(call)),
                    Node::ReturnStmt(ReturnStmt { value: None }),
                ],
            ),
        ],
    });
    let mut check = MapCheckInfo::new();
    let Node::Program(p) = &program else {
        panic!("root must be a Program node");
    };
    let Node::FuncDecl(f) = &p.decls[1] else {
        panic!("decl 1 must be a function");
    };
    let Node::ExprStmt(e) = &f.body[0] else {
        panic!("body 0 must be an expression statement");
    };
    let Node::CallExpr(c) = &**e else {
        panic!("body 0 must be a call");
    };
    check.add_expr_type(&c.args[0], arr_ty(prim(Primitive::I64), 3));
    let out = sla_codegen_text::generate(&program, &check)
        .expect("call with array literal argument must emit on the text backend");
    assert!(
        out.contains("alloc 3"),
        "expected the argument laid out as [u8; 3] (3 bytes), got:\n{out}"
    );
    assert!(
        !out.contains("alloc 24"),
        "the argument must not be laid out at i64 stride, got:\n{out}"
    );
}
