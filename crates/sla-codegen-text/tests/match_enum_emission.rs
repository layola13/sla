//! Regression (RUST-TEXT-EMITTER stage 4): a `match` over a user
//! enum must emit through the text backend (tag dispatch +
//! payload binding loads) instead of stopping at the
//! `expression kind "match_expr"` catch-all.
//!
//! On the pre-fix code `generate` returns
//! `Unsupported { feature: "expression kind \"match_expr\"" }`;
//! on the fixed code it returns the emitted program.

use sla_ast::{
    BinaryExpr, BinaryOp, EnumDecl, EnumPattern, EnumVariant, Field, FuncDecl, Literal, MatchCase,
    MatchExpr, Node, Param, Primitive, Program, ReturnStmt, Type,
};
use sla_lowering::MapCheckInfo;

fn i32t() -> Type {
    Type::Primitive(Primitive::I32)
}

fn pick_ty() -> Type {
    Type::UserDefined(sla_ast::UserDefined {
        name: "Pick".to_string(),
        generics: Vec::new(),
    })
}

#[test]
fn match_over_user_enum_emits_tag_dispatch() {
    // enum Pick { Have { x: i32 }, Lack }
    // fn f(p: Pick) -> i32 {
    //     return match p {
    //         Pick::Have { x } => { x + 1 },
    //         Pick::Lack => { -1 },
    //     };
    // }
    let x_plus_1 = Node::BinaryExpr(BinaryExpr {
        op: BinaryOp::Add,
        left: Box::new(Node::Identifier("x".to_string())),
        right: Box::new(Node::Literal(Literal::IntVal(1))),
    });
    let match_expr = Node::MatchExpr(MatchExpr {
        val: Box::new(Node::Identifier("p".to_string())),
        cases: vec![
            MatchCase {
                pattern: EnumPattern {
                    enum_name: "Pick".to_string(),
                    variant_name: "Have".to_string(),
                    bindings: vec!["x".to_string()],
                },
                guard: None,
                body: vec![Node::ExprStmt(Box::new(x_plus_1))],
            },
            MatchCase {
                pattern: EnumPattern {
                    enum_name: "Pick".to_string(),
                    variant_name: "Lack".to_string(),
                    bindings: Vec::new(),
                },
                guard: None,
                body: vec![Node::ExprStmt(Box::new(Node::Literal(
                    Literal::IntVal(-1),
                )))],
            },
        ],
    });
    let program = Node::Program(Program {
        decls: vec![
            Node::EnumDecl(EnumDecl {
                name: "Pick".to_string(),
                generics: Vec::new(),
                variants: vec![
                    EnumVariant {
                        name: "Have".to_string(),
                        fields: vec![Field {
                            name: "x".to_string(),
                            ty: Box::new(i32t()),
                        }],
                        discriminant: None,
                    },
                    EnumVariant {
                        name: "Lack".to_string(),
                        fields: Vec::new(),
                        discriminant: None,
                    },
                ],
            }),
            Node::FuncDecl(FuncDecl {
                name: "f".to_string(),
                is_pub: false,
                is_extern: false,
                abi: None,
                no_mangle: false,
                is_decl_only: false,
                generics: Vec::new(),
                params: vec![Param {
                    name: "p".to_string(),
                    ty: Box::new(pick_ty()),
                    is_borrow: false,
                    is_move: false,
                    fallible_sugar: false,
                    is_var: false,
                }],
                ret_ty: Box::new(i32t()),
                body: vec![Node::ReturnStmt(ReturnStmt {
                    value: Some(Box::new(match_expr)),
                })],
                is_inline: false,
                is_async: false,
                operator: None,
                is_arena: false,
                ret_fallible_sugar: false,
                ret_borrow_origin: None,
                is_variadic: false,
            }),
        ],
    });

    // Checker facts on the in-place nodes: scrutinee is the enum,
    // the match and both arm tails are i32, the bound identifier
    // and the literal are i32.
    let mut check = MapCheckInfo::new();
    if let Node::Program(prog) = &program {
        if let Node::FuncDecl(f) = &prog.decls[1] {
            if let Node::ReturnStmt(r) = &f.body[0] {
                let m = r.value.as_deref().unwrap();
                check.add_expr_type(m, i32t());
                if let Node::MatchExpr(me) = m {
                    check.add_expr_type(&me.val, pick_ty());
                    if let Node::ExprStmt(inner) = &me.cases[0].body[0] {
                        check.add_expr_type(&me.cases[0].body[0], i32t());
                        check.add_expr_type(inner, i32t());
                        if let Node::BinaryExpr(b) = &**inner {
                            check.add_expr_type(&b.left, i32t());
                            check.add_expr_type(&b.right, i32t());
                        }
                    }
                    check.add_expr_type(&me.cases[1].body[0], i32t());
                    if let Node::ExprStmt(inner) = &me.cases[1].body[0] {
                        check.add_expr_type(inner, i32t());
                    }
                }
            }
        }
    }

    let out = sla_codegen_text::generate(&program, &check)
        .expect("match over a user enum must emit on the text backend");
    assert!(
        out.contains("L_MATCH_TEST") && out.contains("= eq "),
        "expected tag-dispatch emission, got:\n{out}"
    );
    assert!(
        out.contains("load ") && out.contains("+8 as i32"),
        "expected the payload load for binding `x` at offset 8, got:\n{out}"
    );
}
