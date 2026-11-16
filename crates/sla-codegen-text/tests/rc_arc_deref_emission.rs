//! Regression (RUST-TEXT-EMITTER stage 7, RC_GET/ARC_GET
//! cluster): dereferencing an `Rc<T>` / `Arc<T>` must emit the
//! std-surface projection `EXPAND RC_GET` / `EXPAND ARC_GET`
//! (codegen.zig:13075-13086), keyed on lowering_rules
//! rcInnerType/arcInnerType (borrow/pointer peel, then `Name<T>`
//! with exactly one generic). When the deref's inner expression
//! is a release-owning temp (a field read, per
//! lowering_rules exprResultNeedsRelease), the temp is released
//! after the EXPAND, exactly like the plain load path.
//!
//! Pre-fix, `generate` rejected every Rc/Arc deref with
//! `deref of Rc/Arc (RC_GET/ARC_GET family)`; the two controls
//! (Box deref, raw-pointer deref) keep the plain load path and
//! pass both pre- and post-fix.

use sla_ast::{
    DerefExpr, Field, FieldExpr, FuncDecl, Node, Param, Primitive, Program, ReturnStmt, StructDecl,
    Type, UserDefined,
};
use sla_lowering::MapCheckInfo;

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
}

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

fn rc_i64_ty() -> Type {
    ud("Rc", vec![i64_ty()])
}

fn arc_i64_ty() -> Type {
    ud("Arc", vec![i64_ty()])
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

fn deref(inner: Node) -> Node {
    Node::DerefExpr(DerefExpr {
        expr: Box::new(inner),
    })
}

/// The return statement's value node of decl `idx`.
fn return_value_of<'p>(program: &'p Node, idx: usize) -> &'p Node {
    let Node::Program(p) = program else {
        panic!("root must be a Program node");
    };
    let Node::FuncDecl(f) = &p.decls[idx] else {
        panic!("decl {idx} must be a function");
    };
    let Node::ReturnStmt(r) = &f.body[0] else {
        panic!("body 0 must be a return statement");
    };
    r.value.as_deref().expect("return must carry a value")
}

/// fn f(a: Rc<i64>) -> i64 { return *a; }
fn deref_ident_program(handle_ty: Type) -> Node {
    Node::Program(Program {
        decls: vec![func(
            "f",
            vec![param("a", handle_ty)],
            i64_ty(),
            vec![Node::ReturnStmt(ReturnStmt {
                value: Some(Box::new(deref(ident("a")))),
            })],
        )],
    })
}

fn deref_ident_check(program: &Node, handle_ty: Type) -> MapCheckInfo {
    let mut check = MapCheckInfo::new();
    let value = return_value_of(program, 0);
    let Node::DerefExpr(d) = value else {
        panic!("return value must be a deref");
    };
    check.add_expr_type(&d.expr, handle_ty);
    check.add_expr_type(value, i64_ty());
    check
}

#[test]
fn deref_of_rc_identifier_emits_rc_get() {
    let program = deref_ident_program(rc_i64_ty());
    let check = deref_ident_check(&program, rc_i64_ty());
    let out = sla_codegen_text::generate(&program, &check)
        .expect("deref of Rc must emit on the text backend");
    assert!(
        out.contains("EXPAND RC_GET "),
        "expected the RC_GET surface projection, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND ARC_GET "),
        "an Rc deref must not emit ARC_GET, got:\n{out}"
    );
    // An identifier inner is not a release-owning temp: the
    // deref itself must not release the binding (a later
    // function-exit cleanup is a separate matter).
    let get_pos = out.find("EXPAND RC_GET ").expect("RC_GET present");
    assert!(
        !out[get_pos..]
            .lines()
            .nth(1)
            .is_some_and(|l| l.trim_start() == "!a"),
        "the Rc binding itself must not be released by the deref, got:\n{out}"
    );
}

#[test]
fn deref_of_arc_identifier_emits_arc_get() {
    let program = deref_ident_program(arc_i64_ty());
    let check = deref_ident_check(&program, arc_i64_ty());
    let out = sla_codegen_text::generate(&program, &check)
        .expect("deref of Arc must emit on the text backend");
    assert!(
        out.contains("EXPAND ARC_GET "),
        "expected the ARC_GET surface projection, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND RC_GET "),
        "an Arc deref must not emit RC_GET, got:\n{out}"
    );
    let get_pos = out.find("EXPAND ARC_GET ").expect("ARC_GET present");
    assert!(
        !out[get_pos..]
            .lines()
            .nth(1)
            .is_some_and(|l| l.trim_start() == "!a"),
        "the Arc binding itself must not be released by the deref, got:\n{out}"
    );
}

#[test]
fn deref_of_rc_field_temp_emits_rc_get_and_releases_temp() {
    // struct Holder { rc: Rc<i64> }
    // fn f(h: Holder) -> i64 { return *h.rc; }
    // The field read materialises a handle temp; the deref must
    // project it with RC_GET and then release the temp (a field
    // expression is a release-owning result), or the temp leaks.
    let holder_ty = ud("Holder", Vec::new());
    let program = Node::Program(Program {
        decls: vec![
            Node::StructDecl(StructDecl {
                name: "Holder".to_string(),
                derives: Vec::new(),
                generics: Vec::new(),
                fields: vec![Field {
                    name: "rc".to_string(),
                    ty: Box::new(rc_i64_ty()),
                }],
                is_union: false,
                is_opaque: false,
            }),
            func(
                "f",
                vec![param("h", holder_ty.clone())],
                i64_ty(),
                vec![Node::ReturnStmt(ReturnStmt {
                    value: Some(Box::new(deref(Node::FieldExpr(FieldExpr {
                        expr: Box::new(ident("h")),
                        field_name: "rc".to_string(),
                    })))),
                })],
            ),
        ],
    });
    let mut check = MapCheckInfo::new();
    check.add_struct(StructDecl {
        name: "Holder".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
        fields: vec![Field {
            name: "rc".to_string(),
            ty: Box::new(rc_i64_ty()),
        }],
        is_union: false,
        is_opaque: false,
    });
    let value = return_value_of(&program, 1);
    let Node::DerefExpr(d) = value else {
        panic!("return value must be a deref");
    };
    let Node::FieldExpr(fe) = &*d.expr else {
        panic!("deref inner must be a field access");
    };
    check.add_expr_type(&fe.expr, holder_ty);
    check.add_expr_type(&d.expr, rc_i64_ty());
    check.add_expr_type(value, i64_ty());
    let out = sla_codegen_text::generate(&program, &check)
        .expect("deref of an Rc field temp must emit on the text backend");
    let get_pos = out
        .find("EXPAND RC_GET ")
        .expect("expected the RC_GET surface projection");
    let after = &out[get_pos..];
    assert!(
        after.lines().nth(1).is_some_and(|l| l.trim_start().starts_with('!')),
        "the field temp must be released right after the RC_GET, got:\n{out}"
    );
}

#[test]
fn deref_of_box_keeps_plain_load_control() {
    // fn f(b: Box<i64>) -> i64 { return *b; }
    // Zig's deref arm intercepts only Rc/Arc; a Box deref falls
    // through to the plain load. Control: passes pre- and
    // post-fix, and must not grow an EXPAND projection.
    let box_ty = ud("Box", vec![i64_ty()]);
    let program = deref_ident_program(box_ty.clone());
    let check = deref_ident_check(&program, box_ty);
    let out = sla_codegen_text::generate(&program, &check)
        .expect("deref of Box must emit on the text backend");
    assert!(
        out.contains("load "),
        "expected the plain load path for a Box deref, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND RC_GET ") && !out.contains("EXPAND ARC_GET "),
        "a Box deref must not emit an Rc/Arc projection, got:\n{out}"
    );
}

#[test]
fn deref_of_raw_ptr_keeps_plain_load_control() {
    // fn f(p: ptr) -> i64 { return *p; }
    let ptr_ty = Type::Primitive(Primitive::RawPtr);
    let program = deref_ident_program(ptr_ty.clone());
    let check = deref_ident_check(&program, ptr_ty);
    let out = sla_codegen_text::generate(&program, &check)
        .expect("deref of a raw pointer must emit on the text backend");
    assert!(
        out.contains("load "),
        "expected the plain load path for a raw-pointer deref, got:\n{out}"
    );
    assert!(
        !out.contains("EXPAND RC_GET ") && !out.contains("EXPAND ARC_GET "),
        "a raw-pointer deref must not emit an Rc/Arc projection, got:\n{out}"
    );
}
