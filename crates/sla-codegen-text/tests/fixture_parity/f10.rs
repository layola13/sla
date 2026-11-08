//! Fixture 10 — `copied` on Option must emit OPTION_COPIED_U64
//! (codegen.zig:14018), not fall through to the generic catch-all as
//! a bare `EXPAND copied` (which the host assembler rejects: there
//! is no [MACRO] copied in sa_std).
//!
//! Corpus carrier: test_unit_hashmap_xtalk_overwrite, whose sa arm
//! died at generated line 155 `EXPAND copied tmp_26, tmp_25` right
//! after the map TRY_GET_OPTION expansion once the len wall was
//! fixed.
//!
//! Registration note: CheckInfo keys are node ADDRESSES
//! (sla-lowering `node_key` = pointer), so the receiver-type
//! override must be registered on the in-tree node.

use sla_ast::{LetStmt, Node, Primitive, Type, UserDefined};
use sla_lowering::MapCheckInfo;

use super::{call, expr_stmt, func, ident, int, program, register_expr_types, ret, test_decl};

fn option_ty(inner: Type) -> Type {
    Type::UserDefined(UserDefined {
        name: "Option".to_string(),
        generics: vec![inner],
    })
}

pub fn assert_unwrap_or_default_dispatch() {
    let prog = program(vec![
        func(
            "main",
            &[],
            vec![
                expr_stmt(call("unwrap_or_default", vec![ident("o")])),
                ret(int(0)),
            ],
        ),
        test_decl("fixture 10 unwrap_or_default dispatch"),
    ]);
    let mut info = MapCheckInfo::new();
    register_expr_types(&mut info, &prog);
    // Same address-keyed override as the copied case: the argument
    // (receiver) of unwrap_or_default is an Option<i32>.
    if let Node::Program(p) = &prog {
        for decl in &p.decls {
            if let Node::FuncDecl(f) = decl {
                for stmt in &f.body {
                    if let Node::ExprStmt(e) = stmt {
                        if let Node::CallExpr(c) = &**e {
                            info.add_expr_type(
                                &c.args[0],
                                option_ty(Type::Primitive(Primitive::I32)),
                            );
                        }
                    }
                }
            }
        }
    }
    let out = sla_codegen_text::generate(&prog, &info)
        .unwrap_or_else(|err| panic!("fixture 10 (unwrap_or_default): generate failed: {err}"));
    assert!(
        out.contains("EXPAND OPTION_UNWRAP_OR_DEFAULT "),
        "fixture 10: unwrap_or_default on Option must emit EXPAND OPTION_UNWRAP_OR_DEFAULT"
    );
    assert!(
        !out.contains("EXPAND unwrap_or_default "),
        "fixture 10: bare EXPAND unwrap_or_default must not be emitted"
    );
}

pub fn assert_copied_dispatch() {
    let prog = program(vec![
        func(
            "main",
            &[],
            vec![
                Node::LetStmt(LetStmt {
                    name: "o".to_string(),
                    ty: Some(Box::new(option_ty(Type::Primitive(Primitive::I32)))),
                    value: Box::new(int(0)),
                }),
                expr_stmt(call("copied", vec![ident("o")])),
                ret(int(0)),
            ],
        ),
        test_decl("fixture 10 copied dispatch"),
    ]);
    let mut info = MapCheckInfo::new();
    register_expr_types(&mut info, &prog);
    // The harness types every expression as i32; override the copied
    // argument (the receiver) with Option<i32>, as the real checker
    // would record it, on the in-tree node itself.
    if let Node::Program(p) = &prog {
        for decl in &p.decls {
            if let Node::FuncDecl(f) = decl {
                for stmt in &f.body {
                    if let Node::ExprStmt(e) = stmt {
                        if let Node::CallExpr(c) = &**e {
                            info.add_expr_type(
                                &c.args[0],
                                option_ty(Type::Primitive(Primitive::I32)),
                            );
                        }
                    }
                }
            }
        }
    }
    let out = sla_codegen_text::generate(&prog, &info)
        .unwrap_or_else(|err| panic!("fixture 10: generate failed: {err}"));
    assert!(
        out.contains("EXPAND OPTION_COPIED_U64 "),
        "fixture 10: copied on Option must emit EXPAND OPTION_COPIED_U64"
    );
    assert!(
        !out.contains("EXPAND copied "),
        "fixture 10: bare EXPAND copied must not be emitted"
    );
}
