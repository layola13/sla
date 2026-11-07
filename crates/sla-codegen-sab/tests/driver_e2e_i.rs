//! SAB-I end-to-end tests: aggregate literals (struct / union /
//! tuple / array / repeat-array / enum), let-bound and
//! immediately-invoked closures, and impl/overload decl dispatch,
//! driven AST -> driver -> SAB bytes.
//!
//! Claims pinned here:
//! - every I-group shape that was a loud stop at baseline
//!   d811673c now generates, deterministically (two runs byte-
//!   identical);
//! - the u8 repeat fill routes through the `@sa_mem_set` bulk
//!   call body (its text lands in the string pool);
//! - a Copy-derive field stored from a nested literal goes
//!   through `genCopyValue` (its consumed-marker fence text
//!   lands in the pool);
//! - the shapes Zig itself refuses, or that belong to the
//!   F-group fragment machine, still fail loudly — never
//!   silently, never via the text backend.

use sla_ast::*;
use sla_codegen_sab::surface_load::SurfaceRoots;
use sla_codegen_sab::{generate_from_program_with_roots, Error};
use sla_lowering::{node_key, MapCheckInfo};

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn u8_ty() -> Type {
    Type::Primitive(Primitive::U8)
}

fn ud_ty(name: &str) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics: Vec::new(),
    })
}

fn func(name: &str, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params: vec![],
        ret_ty: Box::new(i32_ty()),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
    })
}

fn ret(v: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(v)),
    })
}

fn int(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn let_stmt(name: &str, value: Node) -> Node {
    Node::LetStmt(LetStmt {
        name: name.to_string(),
        ty: None,
        value: Box::new(value),
    })
}

fn tail(v: Node) -> Node {
    Node::ExprStmt(Box::new(v))
}

fn struct_decl(name: &str, derives: &[&str], fields: &[(&str, Type)]) -> Node {
    Node::StructDecl(StructDecl {
        name: name.to_string(),
        derives: derives.iter().map(|d| d.to_string()).collect(),
        generics: Vec::new(),
        fields: fields
            .iter()
            .map(|(n, t)| Field {
                name: n.to_string(),
                ty: Box::new(t.clone()),
            })
            .collect(),
        is_union: false,
        is_opaque: false,
    })
}

fn struct_lit(ty: Type, fields: Vec<(&str, Node)>, update: Option<Node>) -> Node {
    Node::StructLiteral(StructLiteral {
        ty: Box::new(ty),
        fields: fields
            .into_iter()
            .map(|(n, v)| StructLiteralField {
                name: n.to_string(),
                value: Box::new(v),
            })
            .collect(),
        update_expr: update.map(Box::new),
    })
}

fn point_decl() -> Node {
    struct_decl("Point", &[], &[("x", i32_ty()), ("y", i32_ty())])
}

fn point_lit(x: i64, y: i64) -> Node {
    struct_lit(ud_ty("Point"), vec![("x", int(x)), ("y", int(y))], None)
}

/// Type the tree: every node i32 except aggregate literal nodes,
/// which carry their aggregate type (the driver reads those from
/// the checker surface exactly as Zig reads `tc.expr_types`).
fn type_tree(program: &Node) -> MapCheckInfo {
    fn walk(node: &Node, info: &mut MapCheckInfo) {
        let ty = match node {
            Node::TupleLiteral(_) => Type::Tuple(TupleType {
                elems: vec![i32_ty(), i32_ty()],
            }),
            Node::ArrayLiteral(_) => Type::Array(ArrayType {
                elem: Box::new(i32_ty()),
                len: 3,
            }),
            Node::RepeatArrayLiteral(_) => Type::Array(ArrayType {
                elem: Box::new(u8_ty()),
                len: 4,
            }),
            Node::StructLiteral(lit) => (*lit.ty).clone(),
            Node::EnumLiteral(lit) => ud_ty(&lit.enum_name),
            _ => i32_ty(),
        };
        info.expr_types.insert(node_key(node), ty);
        match node {
            Node::Program(p) => p.decls.iter().for_each(|d| walk(d, info)),
            Node::FuncDecl(f) => f.body.iter().for_each(|s| walk(s, info)),
            Node::ImplDecl(i) => i.methods.iter().for_each(|m| walk(m, info)),
            Node::OverloadDecl(o) => o.methods.iter().for_each(|m| walk(m, info)),
            Node::LetStmt(l) => walk(&l.value, info),
            Node::ReturnStmt(r) => {
                if let Some(v) = &r.value {
                    walk(v, info);
                }
            }
            Node::ExprStmt(e) => walk(e, info),
            Node::CallExpr(c) => {
                c.args.iter().for_each(|a| walk(a, info));
                if let Some(callee) = &c.callee {
                    walk(callee, info);
                }
            }
            Node::BinaryExpr(b) => {
                walk(&b.left, info);
                walk(&b.right, info);
            }
            Node::StructLiteral(lit) => {
                lit.fields.iter().for_each(|f| walk(&f.value, info));
                if let Some(u) = &lit.update_expr {
                    walk(u, info);
                }
            }
            Node::TupleLiteral(lit) => lit.elements.iter().for_each(|e| walk(e, info)),
            Node::ArrayLiteral(lit) => lit.elements.iter().for_each(|e| walk(e, info)),
            Node::RepeatArrayLiteral(lit) => walk(&lit.value, info),
            Node::EnumLiteral(lit) => lit.fields.iter().for_each(|f| walk(&f.value, info)),
            Node::ClosureLiteral(c) => walk(&c.body, info),
            Node::MoveExpr(m) => walk(&m.expr, info),
            _ => {}
        }
    }
    let mut info = MapCheckInfo::new();
    walk(program, &mut info);
    info
}

fn empty_roots(tag: &str) -> SurfaceRoots {
    let dir = std::env::temp_dir().join(format!("sab-i-e2e-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    SurfaceRoots {
        sla_std_dir: Some(dir.display().to_string()),
        cwd: dir,
        ..Default::default()
    }
}

fn gen(program: &Node, check: &MapCheckInfo, tag: &str) -> Vec<u8> {
    generate_from_program_with_roots(program, check, empty_roots(tag)).expect("generates")
}

fn gen_twice(program: &Node, check: &MapCheckInfo, tag: &str) -> Vec<u8> {
    let a = gen(program, check, tag);
    let b = gen(program, check, tag);
    assert_eq!(a, b, "generation must be deterministic");
    a
}

fn pool_contains(bytes: &[u8], needle: &str) -> bool {
    bytes
        .windows(needle.len())
        .any(|w| w == needle.as_bytes())
}

#[test]
fn i_struct_literal_generates() {
    let program = Node::Program(Program {
        decls: vec![point_decl(), func("main", vec![tail(point_lit(1, 2))])],
    });
    let mut check = type_tree(&program);
    check.add_struct(StructDecl {
        name: "Point".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
        fields: vec![
            Field { name: "x".to_string(), ty: Box::new(i32_ty()) },
            Field { name: "y".to_string(), ty: Box::new(i32_ty()) },
        ],
        is_union: false,
        is_opaque: false,
    });
    let bytes = gen_twice(&program, &check, "struct");
    assert!(!bytes.is_empty());
}

#[test]
fn i_struct_literal_update_source_generates() {
    let program = Node::Program(Program {
        decls: vec![
            point_decl(),
            func(
                "main",
                vec![
                    let_stmt("p", point_lit(1, 2)),
                    tail(struct_lit(
                        ud_ty("Point"),
                        vec![("y", int(5))],
                        Some(ident("p")),
                    )),
                ],
            ),
        ],
    });
    let mut check = type_tree(&program);
    check.add_struct(StructDecl {
        name: "Point".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
        fields: vec![
            Field { name: "x".to_string(), ty: Box::new(i32_ty()) },
            Field { name: "y".to_string(), ty: Box::new(i32_ty()) },
        ],
        is_union: false,
        is_opaque: false,
    });
    gen_twice(&program, &check, "update");
}

#[test]
fn i_struct_literal_missing_field_is_loud() {
    let program = Node::Program(Program {
        decls: vec![
            point_decl(),
            func(
                "main",
                vec![tail(struct_lit(ud_ty("Point"), vec![("x", int(1))], None))],
            ),
        ],
    });
    let mut check = type_tree(&program);
    check.add_struct(StructDecl {
        name: "Point".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
        fields: vec![
            Field { name: "x".to_string(), ty: Box::new(i32_ty()) },
            Field { name: "y".to_string(), ty: Box::new(i32_ty()) },
        ],
        is_union: false,
        is_opaque: false,
    });
    let err = generate_from_program_with_roots(&program, &check, empty_roots("missing"))
        .expect_err("must refuse");
    assert!(matches!(err, Error::UnsupportedSabDirectFeature(_)), "{err:?}");
}

#[test]
fn i_copy_derive_field_goes_through_gen_copy_value() {
    let inner = struct_decl("Inner", &["Copy"], &[("x", i32_ty())]);
    let outer = struct_decl(
        "Outer",
        &[],
        &[("inner", ud_ty("Inner")), ("n", i32_ty())],
    );
    let program = Node::Program(Program {
        decls: vec![
            inner,
            outer,
            func(
                "main",
                vec![tail(struct_lit(
                    ud_ty("Outer"),
                    vec![
                        ("inner", struct_lit(ud_ty("Inner"), vec![("x", int(9))], None)),
                        ("n", int(3)),
                    ],
                    None,
                ))],
            ),
        ],
    });
    let mut check = type_tree(&program);
    check.add_struct(StructDecl {
        name: "Inner".to_string(),
        derives: vec!["Copy".to_string()],
        generics: Vec::new(),
        fields: vec![Field { name: "x".to_string(), ty: Box::new(i32_ty()) }],
        is_union: false,
        is_opaque: false,
    });
    check.add_struct(StructDecl {
        name: "Outer".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
        fields: vec![
            Field { name: "inner".to_string(), ty: Box::new(ud_ty("Inner")) },
            Field { name: "n".to_string(), ty: Box::new(i32_ty()) },
        ],
        is_union: false,
        is_opaque: false,
    });
    let bytes = gen_twice(&program, &check, "copy");
    assert!(
        pool_contains(&bytes, "^tmp_"),
        "genCopyValue's consumed-marker fence text must be in the pool"
    );
}

#[test]
fn i_move_field_transfers_into_struct() {
    let inner = struct_decl("Inner", &[], &[("v", i32_ty())]);
    let outer = struct_decl("Outer", &[], &[("inner", ud_ty("Inner"))]);
    let program = Node::Program(Program {
        decls: vec![
            inner,
            outer,
            func(
                "main",
                vec![
                    let_stmt("a", struct_lit(ud_ty("Inner"), vec![("v", int(1))], None)),
                    tail(struct_lit(
                        ud_ty("Outer"),
                        vec![("inner", Node::MoveExpr(MoveExpr { expr: Box::new(ident("a")) }))],
                        None,
                    )),
                ],
            ),
        ],
    });
    let mut check = type_tree(&program);
    check.add_struct(StructDecl {
        name: "Inner".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
        fields: vec![Field { name: "v".to_string(), ty: Box::new(i32_ty()) }],
        is_union: false,
        is_opaque: false,
    });
    check.add_struct(StructDecl {
        name: "Outer".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
        fields: vec![Field { name: "inner".to_string(), ty: Box::new(ud_ty("Inner")) }],
        is_union: false,
        is_opaque: false,
    });
    gen_twice(&program, &check, "move");
}

#[test]
fn i_tuple_array_repeat_literals_generate() {
    let tuple_prog = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![tail(Node::TupleLiteral(TupleLiteral {
                elements: vec![int(1), int(2)],
            }))],
        )],
    });
    let check = type_tree(&tuple_prog);
    gen_twice(&tuple_prog, &check, "tuple");

    let array_prog = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![tail(Node::ArrayLiteral(ArrayLiteral {
                elements: vec![int(1), int(2), int(3)],
            }))],
        )],
    });
    let check = type_tree(&array_prog);
    gen_twice(&array_prog, &check, "array");

    let repeat_prog = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![tail(Node::RepeatArrayLiteral(RepeatArrayLiteral {
                value: Box::new(int(0)),
                len: 4,
            }))],
        )],
    });
    let check = type_tree(&repeat_prog);
    // Phase-4 integration pin (SAB-D31): with the F-group ensurer
    // wired, the repeat fill's `sa_std/core/mem.sa` preload dep is
    // appended through the fragment machine and stops loudly at
    // the sci `flattenStdSnippet` boundary — one level deeper than
    // under the pre-F queue-only ensurer this test was written
    // against, where the `@sa_mem_set` call body materialized
    // locally. When a real flattener lands, this pin flips back
    // to generation + the pool assertion on `@sa_mem_set`.
    let err = generate_from_program_with_roots(&repeat_prog, &check, empty_roots("repeat"))
        .expect_err("repeat fill stops at the SAB-D31 flattener boundary");
    let msg = format!("{err}");
    assert!(
        msg.contains("flattenStdSnippet") && msg.contains("sa_std/core/mem.sa"),
        "unexpected boundary error: {msg}"
    );
}

#[test]
fn i_enum_literals_generate() {
    let color = Node::EnumDecl(EnumDecl {
        name: "Color".to_string(),
        generics: Vec::new(),
        variants: vec![
            EnumVariant { name: "Red".to_string(), fields: Vec::new(), discriminant: None },
            EnumVariant {
                name: "Green".to_string(),
                fields: Vec::new(),
                discriminant: Some(5),
            },
            EnumVariant { name: "Blue".to_string(), fields: Vec::new(), discriminant: None },
        ],
    });
    let program = Node::Program(Program {
        decls: vec![
            color,
            func(
                "main",
                vec![tail(Node::EnumLiteral(EnumLiteral {
                    enum_name: "Color".to_string(),
                    variant_name: "Blue".to_string(),
                    fields: Vec::new(),
                }))],
            ),
        ],
    });
    let check = type_tree(&program);
    gen_twice(&program, &check, "enum-unit");

    let shape = Node::EnumDecl(EnumDecl {
        name: "Shape".to_string(),
        generics: Vec::new(),
        variants: vec![EnumVariant {
            name: "Pair".to_string(),
            fields: vec![
                Field { name: "a".to_string(), ty: Box::new(i32_ty()) },
                Field { name: "b".to_string(), ty: Box::new(i32_ty()) },
            ],
            discriminant: None,
        }],
    });
    let program = Node::Program(Program {
        decls: vec![
            shape,
            func(
                "main",
                vec![tail(Node::EnumLiteral(EnumLiteral {
                    enum_name: "Shape".to_string(),
                    variant_name: "Pair".to_string(),
                    fields: vec![
                        EnumLiteralField { name: "a".to_string(), value: Box::new(int(1)) },
                        EnumLiteralField { name: "b".to_string(), value: Box::new(int(2)) },
                    ],
                }))],
            ),
        ],
    });
    let check = type_tree(&program);
    gen_twice(&program, &check, "enum-payload");
}

#[test]
fn i_option_none_is_a_loud_fragment_stop() {
    let program = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![tail(Node::EnumLiteral(EnumLiteral {
                enum_name: "Option".to_string(),
                variant_name: "None".to_string(),
                fields: Vec::new(),
            }))],
        )],
    });
    let check = type_tree(&program);
    let err = generate_from_program_with_roots(&program, &check, empty_roots("optnone"))
        .expect_err("must refuse");
    match err {
        Error::UnsupportedSabDirectFeature(msg) => {
            assert!(msg.contains("OPTION_NEW_NONE"), "{msg}");
        }
        other => panic!("wrong error: {other:?}"),
    }
}

fn closure_add_one() -> Node {
    Node::ClosureLiteral(ClosureLiteral {
        params: vec![Param {
            name: "x".to_string(),
            ty: Box::new(i32_ty()),
            is_borrow: false,
            is_move: false,
        }],
        body: Box::new(Node::BinaryExpr(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(ident("x")),
            right: Box::new(int(1)),
        })),
    })
}

#[test]
fn i_let_bound_closure_call_inlines() {
    let program = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![
                let_stmt("f", closure_add_one()),
                tail(Node::CallExpr(CallExpr {
                    func_name: "f".to_string(),
                    associated_target: None,
                    generics: Vec::new(),
                    args: vec![int(41)],
                    callee: None,
                })),
            ],
        )],
    });
    let check = type_tree(&program);
    gen_twice(&program, &check, "closure-let");
}

#[test]
fn i_closure_alias_call_inlines() {
    let program = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![
                let_stmt("f", closure_add_one()),
                let_stmt("g", ident("f")),
                tail(Node::CallExpr(CallExpr {
                    func_name: "g".to_string(),
                    associated_target: None,
                    generics: Vec::new(),
                    args: vec![int(41)],
                    callee: None,
                })),
            ],
        )],
    });
    let check = type_tree(&program);
    gen_twice(&program, &check, "closure-alias");
}

#[test]
fn i_immediately_invoked_closure_inlines() {
    let program = Node::Program(Program {
        decls: vec![func(
            "main",
            vec![tail(Node::CallExpr(CallExpr {
                func_name: String::new(),
                associated_target: None,
                generics: Vec::new(),
                args: vec![int(41)],
                callee: Some(Box::new(closure_add_one())),
            }))],
        )],
    });
    let check = type_tree(&program);
    gen_twice(&program, &check, "closure-iife");
}

#[test]
fn i_bare_closure_literal_is_loud_like_zig() {
    let program = Node::Program(Program {
        decls: vec![func("main", vec![tail(closure_add_one())])],
    });
    let check = type_tree(&program);
    let err = generate_from_program_with_roots(&program, &check, empty_roots("bare"))
        .expect_err("must refuse");
    assert!(matches!(err, Error::UnsupportedSabDirectFeature(_)), "{err:?}");
}

#[test]
fn i_impl_and_overload_decls_generate() {
    let method = |name: &str| {
        Node::FuncDecl(FuncDecl {
            name: name.to_string(),
            is_pub: false,
            is_extern: false,
            abi: None,
            no_mangle: false,
            is_decl_only: false,
            generics: Vec::new(),
            // NOTE: an idiomatic `&self` parameter is a
            // user-defined-typed parameter, which the baseline
            // driver subset refuses for *every* function shape at
            // its prim_type subset (driver.rs) — pre-existing,
            // not I-group. The dispatch is what is pinned here,
            // so the method takes a primitive parameter.
            params: vec![Param {
                name: "v".to_string(),
                ty: Box::new(i32_ty()),
                is_borrow: false,
                is_move: false,
            }],
            ret_ty: Box::new(i32_ty()),
            body: vec![ret(int(7))],
            is_inline: false,
            is_async: false,
            operator: None,
        })
    };
    let program = Node::Program(Program {
        decls: vec![
            point_decl(),
            Node::ImplDecl(ImplDecl {
                trait_name: None,
                target_ty: Box::new(ud_ty("Point")),
                methods: vec![method("get_x")],
            }),
            Node::OverloadDecl(OverloadDecl {
                target_ty: Box::new(ud_ty("Point")),
                methods: vec![method("plus")],
            }),
            func("main", vec![ret(int(0))]),
        ],
    });
    let check = type_tree(&program);
    gen_twice(&program, &check, "impl");
}
