//! Phase-3 tests, part B (CHK3 item 1): associated calls and
//! receiver-type derivation. Venue truth: real plugin at head
//! 6002b386, `sa sla check` probes (venue 3, 2026-11-07), quoted
//! per test.
//!
//! Shape note: the sla-parser slice does not cover `trait` /
//! `impl` declarations yet ("trait declarations are not covered
//! by the sla-parser slice yet"), so those forms are built as
//! ASTs and driven through `check_program` — the same shape the
//! phase-2 trait tests use. Forms the parser DOES cover (`mod` /
//! `using`, `Option::None`) run the full source pipeline. The
//! Zig-side truth for every AST pin here comes from the venue
//! source probes named in each test, not from the AST itself.

use crate::checker::Checker;
use crate::diag::{Diagnostic, ErrorKind};
use sla_ast::decls::*;
use sla_ast::exprs::*;
use sla_ast::node::Node;
use sla_ast::stmts::*;
use sla_ast::types::*;

fn check_source(source: &str) -> Result<(), Diagnostic> {
    let root = sla_parser::parse_program(source).expect("source must parse");
    let Node::Program(program) = &root else {
        panic!("parser root must be a Program node");
    };
    let mut checker = Checker::new();
    checker.check_program(program)
}

fn check(decls: Vec<Node>) -> Result<(), Diagnostic> {
    let mut checker = Checker::new();
    checker.check_program(&Program { decls })
}

// -- builders ------------------------------------------------------

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn bool_ty() -> Type {
    Type::Primitive(Primitive::Boolean)
}

fn user(name: &str) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics: vec![],
    })
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn int_lit(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn ret_stmt(e: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(e)),
    })
}

fn param(name: &str, ty: Type) -> Param {
    Param {
        name: name.to_string(),
        ty: Box::new(ty),
        is_borrow: false,
        is_move: false,
    }
}

fn borrow_param(name: &str, ty: Type) -> Param {
    Param {
        name: name.to_string(),
        ty: Box::new(ty),
        is_borrow: true,
        is_move: false,
    }
}

fn func(
    name: &str,
    params: Vec<Param>,
    ret_ty: Type,
    body: Vec<Node>,
) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: vec![],
        params,
        ret_ty: Box::new(ret_ty),
        body,
        is_inline: false,
        is_async: false,
        operator: None,
    })
}

fn struct_decl(name: &str, fields: Vec<(&str, Type)>) -> Node {
    Node::StructDecl(StructDecl {
        name: name.to_string(),
        derives: vec![],
        generics: vec![],
        fields: fields
            .into_iter()
            .map(|(n, t)| Field {
                name: n.to_string(),
                ty: Box::new(t),
            })
            .collect(),
        is_union: false,
        is_opaque: false,
    })
}

fn field_of(recv: &str, field: &str) -> Node {
    Node::FieldExpr(FieldExpr {
        expr: Box::new(ident(recv)),
        field_name: field.to_string(),
    })
}

fn dot_call(recv: Node, name: &str, extra: Vec<Node>) -> Node {
    let mut args = vec![recv];
    args.extend(extra);
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: None,
        generics: vec![],
        args,
        callee: None,
    })
}

fn assoc_call(target: &str, name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: Some(target.to_string()),
        generics: vec![],
        args,
        callee: None,
    })
}

fn box2_decl() -> Node {
    struct_decl("Box2", vec![("v", i32_ty())])
}

fn box2_get_method() -> Node {
    func(
        "get",
        vec![param("self", user("Box2"))],
        i32_ty(),
        vec![ret_stmt(field_of("self", "v"))],
    )
}

fn box2_inherent() -> Node {
    Node::ImplDecl(ImplDecl {
        trait_name: None,
        target_ty: Box::new(user("Box2")),
        methods: vec![box2_get_method()],
    })
}

fn host_box2(body: Vec<Node>) -> Node {
    func("f", vec![param("b", user("Box2"))], i32_ty(), body)
}

// -- inherent forms ------------------------------------------------

#[test]
fn inherent_dot_call_resolves() {
    // Venue probe a_inherent_dot: rc=0.
    let result = check(vec![
        box2_decl(),
        box2_inherent(),
        host_box2(vec![ret_stmt(dot_call(ident("b"), "get", vec![]))]),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn inherent_associated_call_resolves() {
    // Venue probe a_inherent_assoc: rc=0 (`Box2::get(b)`).
    let result = check(vec![
        box2_decl(),
        box2_inherent(),
        host_box2(vec![ret_stmt(assoc_call(
            "Box2",
            "get",
            vec![ident("b")],
        ))]),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn borrow_receiver_method_auto_borrows() {
    // Venue probe a_borrow_dot: rc=0 (`&self` method, bare
    // receiver; method paths use auto_borrow_receiver = true).
    let method = func(
        "get",
        vec![borrow_param("self", user("Box2"))],
        i32_ty(),
        vec![ret_stmt(field_of("self", "v"))],
    );
    let imp = Node::ImplDecl(ImplDecl {
        trait_name: None,
        target_ty: Box::new(user("Box2")),
        methods: vec![method],
    });
    let result = check(vec![
        box2_decl(),
        imp,
        host_box2(vec![ret_stmt(dot_call(ident("b"), "get", vec![]))]),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn borrow_typed_receiver_rejected_for_plain_self() {
    // Venue probe a_borrow_recv: rc=1 TypeMismatch. The peel
    // finds the method KEY, but argument checking does not unwrap
    // an identifier whose TYPE is a borrow.
    let host = func(
        "f",
        vec![Param {
            name: "b".to_string(),
            ty: Box::new(Type::Borrow(Box::new(user("Box2")))),
            is_borrow: false,
            is_move: false,
        }],
        i32_ty(),
        vec![ret_stmt(dot_call(ident("b"), "get", vec![]))],
    );
    let result = check(vec![box2_decl(), box2_inherent(), host]);
    let err = result.expect_err("borrow receiver must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn pointer_typed_receiver_rejected_for_plain_self() {
    // Venue probe a_ptr_recv: rc=1 TypeMismatch, same mechanism.
    let host = func(
        "f",
        vec![Param {
            name: "b".to_string(),
            ty: Box::new(Type::Pointer(Box::new(user("Box2")))),
            is_borrow: false,
            is_move: false,
        }],
        i32_ty(),
        vec![ret_stmt(dot_call(ident("b"), "get", vec![]))],
    );
    let result = check(vec![box2_decl(), box2_inherent(), host]);
    let err = result.expect_err("pointer receiver must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn wrong_receiver_type_rejected() {
    // Venue probe a_wrong_recv: rc=1 "Undefined call: get
    // (error.UndefinedVariable)".
    let a1 = struct_decl("A1", vec![("v", i32_ty())]);
    let b1 = struct_decl("B1", vec![("v", i32_ty())]);
    let imp = Node::ImplDecl(ImplDecl {
        trait_name: None,
        target_ty: Box::new(user("A1")),
        methods: vec![func(
            "get",
            vec![param("self", user("A1"))],
            i32_ty(),
            vec![ret_stmt(field_of("self", "v"))],
        )],
    });
    let host = func(
        "f",
        vec![param("b", user("B1"))],
        i32_ty(),
        vec![ret_stmt(dot_call(ident("b"), "get", vec![]))],
    );
    let result = check(vec![a1, b1, imp, host]);
    let err = result.expect_err("wrong receiver must fail");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
    assert!(err.message.contains("Undefined call: get"), "{err:?}");
}

#[test]
fn unknown_associated_call_rejected() {
    // Venue probe a_assoc_unknown: rc=1 "Undefined call: nope
    // (error.UndefinedVariable)".
    let result = check(vec![
        box2_decl(),
        host_box2(vec![ret_stmt(assoc_call(
            "Box2",
            "nope",
            vec![ident("b")],
        ))]),
    ]);
    let err = result.expect_err("unknown associated must fail");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
    assert!(err.message.contains("Undefined call: nope"), "{err:?}");
}

// -- trait forms ----------------------------------------------------

fn dr_trait() -> Node {
    Node::TraitDecl(TraitDecl {
        name: "Dr".to_string(),
        supertraits: vec![],
        methods: vec![TraitMethod {
            name: "draw".to_string(),
            params: vec![borrow_param("self", user("Circ"))],
            ret_ty: Box::new(i32_ty()),
        }],
    })
}

fn circ_decl() -> Node {
    struct_decl("Circ", vec![("r", i32_ty())])
}

fn circ_dr_impl() -> Node {
    Node::ImplDecl(ImplDecl {
        trait_name: Some("Dr".to_string()),
        target_ty: Box::new(user("Circ")),
        methods: vec![func(
            "draw",
            vec![borrow_param("self", user("Circ"))],
            i32_ty(),
            vec![ret_stmt(field_of("self", "r"))],
        )],
    })
}

fn host_circ(body: Vec<Node>) -> Node {
    func("f", vec![param("c", user("Circ"))], i32_ty(), body)
}

#[test]
fn trait_dot_call_resolves() {
    // Venue probe a_trait_dot: rc=0.
    let result = check(vec![
        dr_trait(),
        circ_decl(),
        circ_dr_impl(),
        host_circ(vec![ret_stmt(dot_call(ident("c"), "draw", vec![]))]),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn trait_associated_call_resolves() {
    // Venue probe a_trait_assoc: rc=0 (`Dr::draw(&c)`).
    let borrow_c = Node::BorrowExpr(BorrowExpr {
        expr: Box::new(ident("c")),
    });
    let result = check(vec![
        dr_trait(),
        circ_decl(),
        circ_dr_impl(),
        host_circ(vec![ret_stmt(assoc_call("Dr", "draw", vec![borrow_c]))]),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn type_target_trait_associated_resolves() {
    // Venue probe a_type_trait_assoc: rc=0 (`Pair2::build(1, 2)`
    // via the associated trait fallback; no inherent `build`).
    let fac = Node::TraitDecl(TraitDecl {
        name: "Fac".to_string(),
        supertraits: vec![],
        methods: vec![TraitMethod {
            name: "build".to_string(),
            params: vec![param("a", i32_ty()), param("b", i32_ty())],
            ret_ty: Box::new(user("Pair2")),
        }],
    });
    let pair = struct_decl("Pair2", vec![("a", i32_ty()), ("b", i32_ty())]);
    let lit = Node::StructLiteral(StructLiteral {
        ty: Box::new(user("Pair2")),
        fields: vec![
            StructLiteralField {
                name: "a".to_string(),
                value: Box::new(ident("a")),
            },
            StructLiteralField {
                name: "b".to_string(),
                value: Box::new(ident("b")),
            },
        ],
        update_expr: None,
    });
    let imp = Node::ImplDecl(ImplDecl {
        trait_name: Some("Fac".to_string()),
        target_ty: Box::new(user("Pair2")),
        methods: vec![func(
            "build",
            vec![param("a", i32_ty()), param("b", i32_ty())],
            user("Pair2"),
            vec![ret_stmt(lit)],
        )],
    });
    let host = func(
        "f",
        vec![],
        user("Pair2"),
        vec![ret_stmt(assoc_call(
            "Pair2",
            "build",
            vec![int_lit(1), int_lit(2)],
        ))],
    );
    let result = check(vec![fac, pair, imp, host]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn inherent_beats_trait_for_dot_call() {
    // Venue probe a_priority_ty: rc=0 with an i32 return — had
    // the trait method (bool) won, the return would mismatch.
    let sc = Node::TraitDecl(TraitDecl {
        name: "Sc".to_string(),
        supertraits: vec![],
        methods: vec![TraitMethod {
            name: "score".to_string(),
            params: vec![borrow_param("self", user("Item"))],
            ret_ty: Box::new(bool_ty()),
        }],
    });
    let item = struct_decl("Item", vec![("v", i32_ty())]);
    let sc_impl = Node::ImplDecl(ImplDecl {
        trait_name: Some("Sc".to_string()),
        target_ty: Box::new(user("Item")),
        methods: vec![func(
            "score",
            vec![borrow_param("self", user("Item"))],
            bool_ty(),
            vec![ret_stmt(Node::Literal(Literal::BoolVal(true)))],
        )],
    });
    let inherent = Node::ImplDecl(ImplDecl {
        trait_name: None,
        target_ty: Box::new(user("Item")),
        methods: vec![func(
            "score",
            vec![param("self", user("Item"))],
            i32_ty(),
            vec![ret_stmt(Node::BinaryExpr(BinaryExpr {
                op: BinaryOp::Add,
                left: Box::new(field_of("self", "v")),
                right: Box::new(int_lit(100)),
            }))],
        )],
    });
    let host = func(
        "f",
        vec![param("i", user("Item"))],
        i32_ty(),
        vec![ret_stmt(dot_call(ident("i"), "score", vec![]))],
    );
    let result = check(vec![sc, item, sc_impl, inherent, host]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn ambiguous_trait_method_rejected() {
    // Venue probe a_ambig: rc=1 "Ambiguous trait method `m` for
    // type `S` (error.TypeMismatch)".
    let mk_trait = |name: &str| Node::TraitDecl(TraitDecl {
        name: name.to_string(),
        supertraits: vec![],
        methods: vec![TraitMethod {
            name: "m".to_string(),
            params: vec![borrow_param("self", user("S"))],
            ret_ty: Box::new(i32_ty()),
        }],
    });
    let mk_impl = |tn: &str, v: i64| Node::ImplDecl(ImplDecl {
        trait_name: Some(tn.to_string()),
        target_ty: Box::new(user("S")),
        methods: vec![func(
            "m",
            vec![borrow_param("self", user("S"))],
            i32_ty(),
            vec![ret_stmt(int_lit(v))],
        )],
    });
    let host = func(
        "f",
        vec![param("s", user("S"))],
        i32_ty(),
        vec![ret_stmt(dot_call(ident("s"), "m", vec![]))],
    );
    let result = check(vec![
        mk_trait("T1"),
        mk_trait("T2"),
        struct_decl("S", vec![("v", i32_ty())]),
        mk_impl("T1", 1),
        mk_impl("T2", 2),
        host,
    ]);
    let err = result.expect_err("ambiguous trait method must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
    assert!(
        err.message.contains("Ambiguous trait method `m` for type `S`"),
        "{err:?}"
    );
}

// -- using extensions (source pipeline) ------------------------------

const USING_MATH: &str = "mod math {\n    pub fn double(value: i32) -> i32 {\n        return value + value;\n    }\n}\n";

#[test]
fn using_extension_resolves_on_primitive_receiver() {
    // Venue probe a_using: rc=0.
    let src = format!("{USING_MATH}using math;\nfn f(v: i32) -> i32 {{\n    return v.double();\n}}\n");
    assert!(check_source(&src).is_ok());
}

#[test]
fn extension_without_using_rejected() {
    // Venue probe a_using_none: rc=1 "Undefined call: double
    // (error.UndefinedVariable)".
    let src = format!("{USING_MATH}fn f(v: i32) -> i32 {{\n    return v.double();\n}}\n");
    let err = check_source(&src).expect_err("extension without using must fail");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
    assert!(err.message.contains("Undefined call: double"), "{err:?}");
}

#[test]
fn using_ambiguity_rejected() {
    // Venue probe a_using_ambig: rc=1 "Ambiguous static
    // extension call `double` for type `primitive`
    // (error.AmbiguousCall)".
    let src = "mod ma {\n    pub fn double(value: i32) -> i32 {\n        return value + value;\n    }\n}\nmod mb {\n    pub fn double(value: i32) -> i32 {\n        return value * 2;\n    }\n}\nusing ma;\nusing mb;\nfn f(v: i32) -> i32 {\n    return v.double();\n}\n";
    let err = check_source(src).expect_err("ambiguous extension must fail");
    assert_eq!(err.kind, ErrorKind::AmbiguousCall);
    assert!(
        err.message
            .contains("Ambiguous static extension call `double` for type `primitive`"),
        "{err:?}"
    );
}

#[test]
fn using_extension_with_borrow_type_param() {
    // Venue probe a_using_borrowty: rc=0 — the extension's first
    // parameter is declared with a borrow TYPE (`n: &Num`); an
    // owned receiver matches via the DEF-E0 re-expression.
    // (AST form: the parser slice covers `mod`/`using`, but the
    // struct + module combination is built here for symmetry
    // with the other pins; the module function is registered
    // under its mangled name directly.)
    // The extension resolver consults `funcs` only, and only pub
    // entries: the module function arrives as a pub FuncDecl
    // under its mangled name (the shape the parser produces for
    // `mod ext { pub fn getv ... }`).
    let mut ext_fn = match func(
        "ext__getv",
        vec![Param {
            name: "n".to_string(),
            ty: Box::new(Type::Borrow(Box::new(user("Num")))),
            is_borrow: false,
            is_move: false,
        }],
        i32_ty(),
        vec![ret_stmt(field_of("n", "v"))],
    ) {
        Node::FuncDecl(mut f) => {
            f.is_pub = true;
            Node::FuncDecl(f)
        }
        _ => unreachable!(),
    };
    let _ = &mut ext_fn;
    let host = func(
        "f",
        vec![param("n", user("Num"))],
        i32_ty(),
        vec![ret_stmt(dot_call(ident("n"), "getv", vec![]))],
    );
    let program = Program {
        decls: vec![
            struct_decl("Num", vec![("v", i32_ty())]),
            Node::UsingDecl(UsingDecl {
                path: "ext".to_string(),
            }),
            ext_fn,
            host,
        ],
    };
    let mut checker = Checker::new();
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn option_none_associated_resolves() {
    // Venue probe a_option_none: rc=0.
    let src = "fn f() -> Option<i32> {\n    return Option::None();\n}\n";
    assert!(check_source(src).is_ok());
}

