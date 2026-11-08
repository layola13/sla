//! Fixture 09 (R-4 len dispatch): a free `len(m)` call over a
//! HashMap-typed binding must lower to the std macro `MAP_LEN`
//! (Zig codegen.zig:13884 maps hashMapTypes -> `EXPAND MAP_LEN`),
//! and over a BTreeMap-typed binding to `BTREE_MAP_LEN`
//! (codegen.zig:13891). Before the fix, the Rust free-len dispatch
//! table in surface.rs had no map arms, returned None, and the call
//! fell through to the generic path, which emitted a bare
//! `EXPAND len` that the host assembler rejects (no `[MACRO] len`
//! exists in sa_std) — see corpus test_unit_hashmap_xtalk_overwrite.

use sla_ast::{LetStmt, Node, Primitive, Type, UserDefined};
use sla_lowering::MapCheckInfo;

use super::{call, expr_stmt, func, ident, int, program, register_expr_types, ret, test_decl};

fn map_ty(name: &str) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics: vec![
            Type::Primitive(Primitive::I64),
            Type::Primitive(Primitive::U64),
        ],
    })
}

fn build_for(map_name: &str) -> (Node, MapCheckInfo) {
    let prog = program(vec![
        func(
            "main",
            &[],
            vec![
                Node::LetStmt(LetStmt {
                    name: "m".to_string(),
                    ty: Some(Box::new(map_ty(map_name))),
                    value: Box::new(int(0)),
                }),
                expr_stmt(call("len", vec![ident("m")])),
                ret(int(0)),
            ],
        ),
        test_decl("fixture 09 map len dispatch"),
    ]);
    let mut info = MapCheckInfo::new();
    register_expr_types(&mut info, &prog);
    // The harness types every identifier as i32; override the len
    // argument with the map type the real checker would record.
    // CheckInfo keys are node ADDRESSES (node_key = pointer), so the
    // override must be registered on the in-tree node itself.
    if let Node::Program(p) = &prog {
        for decl in &p.decls {
            if let Node::FuncDecl(f) = decl {
                for stmt in &f.body {
                    if let Node::ExprStmt(e) = stmt {
                        if let Node::CallExpr(c) = &**e {
                            info.add_expr_type(&c.args[0], map_ty(map_name));
                        }
                    }
                }
            }
        }
    }
    (prog, info)
}

pub fn assert_map_len_dispatch() {
    for (map_name, macro_name) in [("HashMap", "MAP_LEN"), ("BTreeMap", "BTREE_MAP_LEN")] {
        let (prog, info) = build_for(map_name);
        let out = sla_codegen_text::generate(&prog, &info)
            .unwrap_or_else(|err| panic!("fixture 09 ({map_name}): generate failed: {err}"));
        assert!(
            out.contains(&format!("EXPAND {macro_name} ")),
            "fixture 09 ({map_name}): len over {map_name} must emit EXPAND {macro_name}"
        );
        assert!(
            !out.contains("EXPAND len "),
            "fixture 09 ({map_name}): must not fall through to a bare EXPAND len"
        );
    }
}

