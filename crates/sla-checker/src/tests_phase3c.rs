//! Phase-3 tests, part C (CHK3 item 1): registration asymmetry
//! pins for the associated / receiver-inferred fetches. These
//! address the checker's symbol tables directly (imported
//! signatures vs funcs), so they are AST-level by construction;
//! the corresponding source-level verdicts are venue-pinned in
//! tests_phase3b.rs.

use crate::checker::{Checker, FuncSig, ParamSig};
use sla_ast::decls::*;
use sla_ast::exprs::*;
use sla_ast::node::Node;
use sla_ast::stmts::*;
use sla_ast::types::*;

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
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

fn host_box2(body: Vec<Node>) -> Node {
    func("f", vec![param("b", user("Box2"))], i32_ty(), body)
}

// -- registration asymmetry pins --------------------------------------

fn psig(name: &str, ty: Type) -> ParamSig {
    ParamSig {
        name: name.to_string(),
        ty,
        is_borrow: false,
        is_move: false,
    }
}

fn imported_sig(params: Vec<ParamSig>, ret_ty: Type) -> FuncSig {
    FuncSig {
        params,
        ret_ty,
        is_pub: true,
    }
}

#[test]
fn associated_static_imported_only_misses_but_funcs_resolves() {
    // Zig's associated inherent fetch consults funcs ONLY, and a
    // static (non-receiver) call form gets no rescue from the
    // receiver-inferred path (args[0] is an i32, not a Box2):
    // an imported-only `Box2_make` does NOT resolve
    // `Box2::make(5)`, while the same symbol in funcs does.
    let make = func(
        "make",
        vec![param("x", i32_ty())],
        user("Box2"),
        vec![ret_stmt(Node::StructLiteral(StructLiteral {
            ty: Box::new(user("Box2")),
            fields: vec![StructLiteralField {
                name: "v".to_string(),
                value: Box::new(ident("x")),
            }],
            update_expr: None,
        }))],
    );
    let host = func(
        "f",
        vec![],
        user("Box2"),
        vec![ret_stmt(assoc_call("Box2", "make", vec![int_lit(5)]))],
    );

    let mut checker = Checker::new();
    checker.register_imported_signature(
        "Box2_make",
        imported_sig(vec![psig("x", i32_ty())], user("Box2")),
    );
    let result = checker.check_program(&Program {
        decls: vec![box2_decl(), host.clone()],
    });
    assert!(result.is_err(), "imported-only must miss: {result:?}");

    let mut checker = Checker::new();
    checker.register_func_as("Box2_make", match &make {
        Node::FuncDecl(f) => f,
        _ => unreachable!(),
    });
    let result = checker.check_program(&Program {
        decls: vec![box2_decl(), host],
    });
    assert!(result.is_ok(), "funcs must resolve: {result:?}");
}

#[test]
fn receiver_inferred_inherent_uses_imported_signatures() {
    // The receiver-inferred inherent path (venue :9669) consults
    // imported signatures too: `b.get()` resolves.
    let mut checker = Checker::new();
    checker.register_imported_signature(
        "Box2_get",
        imported_sig(vec![psig("self", user("Box2"))], i32_ty()),
    );
    let program = Program {
        decls: vec![host_box2(vec![ret_stmt(dot_call(
            ident("b"),
            "get",
            vec![],
        ))])],
    };
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn generic_receiver_key_uses_base_name() {
    // A receiver of `Bag<i32>` derives the key `Bag_get` from the
    // base name (Zig bufPrint of the peeled user-defined name).
    let bag_i32 = Type::UserDefined(UserDefined {
        name: "Bag".to_string(),
        generics: vec![i32_ty()],
    });
    let mut checker = Checker::new();
    checker.register_imported_signature(
        "Bag_get",
        imported_sig(vec![psig("self", bag_i32.clone())], i32_ty()),
    );
    let host = Node::FuncDecl(FuncDecl {
        name: "f".to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: vec![],
        params: vec![Param {
            name: "b".to_string(),
            ty: Box::new(bag_i32),
            is_borrow: false,
            is_move: false,
        }],
        ret_ty: Box::new(i32_ty()),
        body: vec![ret_stmt(dot_call(ident("b"), "get", vec![]))],
        is_inline: false,
        is_async: false,
        operator: None,
    });
    let result = checker.check_program(&Program { decls: vec![host] });
    assert!(result.is_ok(), "{result:?}");
}
