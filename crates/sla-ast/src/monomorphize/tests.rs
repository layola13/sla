//! Unit tests for the monomorphizer port. The first three mirror
//! the Zig original's own tests (`monomorphize generic struct`
//! and the G-4 `TemplateNotFound` evidence tests in
//! monomorphizer.zig); the rest pin the FB-012 behaviors:
//! value-form references, nested / multi-parameter type
//! arguments, explicit and inferred generic calls, and the
//! stand-alone-name rule.

use super::types::mangled_name;
use super::{monomorphize_program, MonomorphizeError};
use crate::decls::{EnumDecl, EnumVariant, Field, FuncDecl, Param, Program, StructDecl};
use crate::exprs::{
    CallExpr, EnumLiteral, EnumLiteralField, FieldExpr, GenericFuncRef, Literal,
};
use crate::node::Node;
use crate::stmts::{LetStmt, ReturnStmt};
use crate::types::{ArrayType, Primitive, TupleType, Type, UserDefined};

fn prim(p: Primitive) -> Type {
    Type::Primitive(p)
}

fn i32_ty() -> Type {
    prim(Primitive::I32)
}

fn bool_ty() -> Type {
    prim(Primitive::Boolean)
}

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
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

fn func(name: &str, generics: &[&str], params: Vec<Param>, ret: Type, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: generics.iter().map(|g| g.to_string()).collect(),
        params,
        ret_ty: Box::new(ret),
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

fn int_lit(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn ret(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(value)),
    })
}

fn expr_stmt(value: Node) -> Node {
    Node::ExprStmt(Box::new(value))
}

fn gref(name: &str, generics: Vec<Type>) -> Node {
    Node::GenericFuncRef(GenericFuncRef {
        func_name: name.to_string(),
        generics,
    })
}

fn call(name: &str, generics: Vec<Type>, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: None,
        generics,
        args,
        callee: None,
    })
}

fn program(decls: Vec<Node>) -> Node {
    Node::Program(Program { decls })
}

fn func_decls(root: &Node) -> Vec<&FuncDecl> {
    let Node::Program(p) = root else { panic!("not a program") };
    p.decls
        .iter()
        .filter_map(|d| match d {
            Node::FuncDecl(f) => Some(f),
            _ => None,
        })
        .collect()
}

fn find_func<'a>(root: &'a Node, name: &str) -> Option<&'a FuncDecl> {
    func_decls(root).into_iter().find(|f| f.name == name)
}

#[test]
fn mangled_name_rules_match_zig() {
    assert_eq!(mangled_name("ident", &[i32_ty()]), "ident_i32");
    assert_eq!(
        mangled_name("pair2", &[i32_ty(), bool_ty()]),
        "pair2_i32_bool"
    );
    assert_eq!(
        mangled_name("wrap", &[ud("Vec", vec![i32_ty()])]),
        "wrap_Vec_i32"
    );
    assert_eq!(
        mangled_name("f", &[Type::Pointer(Box::new(i32_ty()))]),
        "f_ptr_i32"
    );
    assert_eq!(
        mangled_name("f", &[Type::Borrow(Box::new(prim(Primitive::U8)))]),
        "f_ref_u8"
    );
    assert_eq!(
        mangled_name(
            "f",
            &[Type::Array(ArrayType {
                elem: Box::new(prim(Primitive::U8)),
                len: 4
            })]
        ),
        "f_arr4_u8"
    );
    assert_eq!(
        mangled_name(
            "f",
            &[Type::Tuple(TupleType {
                elems: vec![i32_ty(), bool_ty()]
            })]
        ),
        "f_tuple_i32_bool_"
    );
}

#[test]
fn monomorphize_generic_struct() {
    // Mirror of the Zig test of the same name.
    let option = Node::StructDecl(StructDecl {
        name: "Option".to_string(),
        derives: Vec::new(),
        generics: vec!["T".to_string()],
        fields: vec![
            Field {
                name: "has_value".to_string(),
                ty: Box::new(bool_ty()),
            },
            Field {
                name: "value".to_string(),
                ty: Box::new(ud("T", vec![])),
            },
        ],
        is_union: false,
        is_opaque: false,
    });
    let process = func(
        "process",
        &[],
        vec![param("opt", ud("Option", vec![i32_ty()]))],
        i32_ty(),
        vec![ret(Node::FieldExpr(FieldExpr {
            expr: Box::new(ident("opt")),
            field_name: "value".to_string(),
        }))],
    );
    let mut root = program(vec![option, process]);
    monomorphize_program(&mut root).expect("monomorphize must succeed");

    let Node::Program(p) = &root else { panic!("not a program") };
    let instance = p.decls.iter().find_map(|d| match d {
        Node::StructDecl(s) if s.name == "Option_i32" => Some(s),
        _ => None,
    });
    let instance = instance.expect("Option_i32 struct decl must be generated");
    assert_eq!(instance.fields.len(), 2);
    assert_eq!(*instance.fields[1].ty, i32_ty());
    assert!(
        !p.decls
            .iter()
            .any(|d| matches!(d, Node::StructDecl(s) if s.name == "Option")),
        "the template itself must not survive"
    );
    let process = find_func(&root, "process").expect("process stays");
    assert_eq!(process.params[0].ty, Box::new(ud("Option_i32", vec![])));
}

#[test]
fn generic_func_ref_becomes_mangled_identifier() {
    let identity = func(
        "fn_ptr_generic_identity",
        &["T"],
        vec![param("value", ud("T", vec![]))],
        ud("T", vec![]),
        vec![ret(ident("value"))],
    );
    let use_it = func(
        "use_it",
        &[],
        vec![],
        i32_ty(),
        vec![
            Node::LetStmt(LetStmt {
                name: "f".to_string(),
                ty: None,
                value: Box::new(gref("fn_ptr_generic_identity", vec![i32_ty()])),
            }),
            ret(call("f", vec![], vec![int_lit(12)])),
        ],
    );
    let mut root = program(vec![identity, use_it]);
    monomorphize_program(&mut root).expect("monomorphize must succeed");

    assert!(
        find_func(&root, "fn_ptr_generic_identity").is_none(),
        "the template itself must not survive"
    );
    let instance = find_func(&root, "fn_ptr_generic_identity_i32").expect("instance generated");
    assert!(instance.generics.is_empty());
    assert_eq!(instance.params[0].ty, Box::new(i32_ty()));
    assert_eq!(instance.ret_ty, Box::new(i32_ty()));

    let use_it = find_func(&root, "use_it").expect("use_it stays");
    let Node::LetStmt(l) = &use_it.body[0] else {
        panic!("first statement must be the let")
    };
    assert_eq!(
        &*l.value,
        &ident("fn_ptr_generic_identity_i32"),
        "the value reference must become the mangled identifier"
    );
}

#[test]
fn nested_and_multi_param_value_forms() {
    let ident_fn = func(
        "ident",
        &["T"],
        vec![param("x", ud("T", vec![]))],
        ud("T", vec![]),
        vec![ret(ident("x"))],
    );
    let pair2 = func(
        "pair2",
        &["A", "B"],
        vec![param("a", ud("A", vec![])), param("b", ud("B", vec![]))],
        ud("A", vec![]),
        vec![ret(ident("a"))],
    );
    let use_it = func(
        "use_it",
        &[],
        vec![],
        i32_ty(),
        vec![
            expr_stmt(gref("ident", vec![ud("Vec", vec![i32_ty()])])),
            expr_stmt(gref("pair2", vec![i32_ty(), bool_ty()])),
            ret(int_lit(0)),
        ],
    );
    let mut root = program(vec![ident_fn, pair2, use_it]);
    monomorphize_program(&mut root).expect("monomorphize must succeed");
    assert!(find_func(&root, "ident_Vec_i32").is_some());
    let pair = find_func(&root, "pair2_i32_bool").expect("pair2 instance generated");
    assert_eq!(pair.params[1].ty, Box::new(bool_ty()));
}

#[test]
fn generic_func_ref_to_missing_template_is_template_not_found() {
    // Mirror of the Zig G-4 evidence test.
    let main_fn = func(
        "main",
        &[],
        vec![],
        i32_ty(),
        vec![expr_stmt(gref("missing", vec![i32_ty()])), ret(int_lit(0))],
    );
    let mut root = program(vec![main_fn]);
    let err = monomorphize_program(&mut root).expect_err("must fail");
    assert_eq!(err, MonomorphizeError::TemplateNotFound("missing".to_string()));
}

#[test]
fn generic_name_must_stand_alone() {
    let template = func(
        "id",
        &["T"],
        vec![param("x", ud("T", vec![]))],
        ud("T", vec![]),
        vec![ret(ident("x"))],
    );
    let concrete = func(
        "id",
        &[],
        vec![param("x", i32_ty())],
        i32_ty(),
        vec![ret(ident("x"))],
    );
    let mut root = program(vec![template, concrete]);
    let err = monomorphize_program(&mut root).expect_err("must fail");
    assert_eq!(
        err,
        MonomorphizeError::GenericOverloadConflict("id".to_string())
    );
}

#[test]
fn explicit_generic_call_is_rewritten() {
    let id_fn = func(
        "id",
        &["T"],
        vec![param("x", ud("T", vec![]))],
        ud("T", vec![]),
        vec![ret(ident("x"))],
    );
    let main_fn = func(
        "main",
        &[],
        vec![],
        i32_ty(),
        vec![ret(call("id", vec![i32_ty()], vec![int_lit(5)]))],
    );
    let mut root = program(vec![id_fn, main_fn]);
    monomorphize_program(&mut root).expect("monomorphize must succeed");
    assert!(find_func(&root, "id_i32").is_some());
    let main_fn = find_func(&root, "main").expect("main stays");
    let Node::ReturnStmt(r) = &main_fn.body[0] else {
        panic!("body must be the return")
    };
    let Node::CallExpr(c) = &**r.value.as_ref().expect("return value") else {
        panic!("return value must be the call")
    };
    assert_eq!(c.func_name, "id_i32");
    assert!(c.generics.is_empty());
}

#[test]
fn inferred_generic_call_is_rewritten_from_literal() {
    let id_fn = func(
        "id",
        &["T"],
        vec![param("x", ud("T", vec![]))],
        ud("T", vec![]),
        vec![ret(ident("x"))],
    );
    let main_fn = func(
        "main",
        &[],
        vec![],
        i32_ty(),
        vec![ret(call("id", vec![], vec![int_lit(5)]))],
    );
    let mut root = program(vec![id_fn, main_fn]);
    monomorphize_program(&mut root).expect("monomorphize must succeed");
    assert!(
        find_func(&root, "id_i32").is_some(),
        "an int literal infers i32, exactly as in Zig"
    );
}

#[test]
fn uninferable_generic_call_is_left_ordinary() {
    let id_fn = func(
        "id",
        &["T"],
        vec![param("x", ud("T", vec![]))],
        ud("T", vec![]),
        vec![ret(ident("x"))],
    );
    let main_fn = func(
        "main",
        &[],
        vec![param("x", i32_ty())],
        i32_ty(),
        vec![ret(call("id", vec![], vec![ident("x")]))],
    );
    let mut root = program(vec![id_fn, main_fn]);
    monomorphize_program(&mut root).expect("monomorphize must succeed");
    assert!(find_func(&root, "id_i32").is_none());
    let main_fn = find_func(&root, "main").expect("main stays");
    let Node::ReturnStmt(r) = &main_fn.body[0] else {
        panic!("body must be the return")
    };
    let Node::CallExpr(c) = &**r.value.as_ref().expect("return value") else {
        panic!("return value must be the call")
    };
    assert_eq!(c.func_name, "id", "an uninferable call is not rewritten");
}

#[test]
fn enum_literal_renamed_for_specialized_let() {
    let option = Node::EnumDecl(EnumDecl {
        name: "Option".to_string(),
        generics: vec!["T".to_string()],
        variants: vec![
            EnumVariant {
                name: "Some".to_string(),
                fields: vec![Field {
                    name: "value".to_string(),
                    ty: Box::new(ud("T", vec![])),
                }],
                discriminant: None,
            },
            EnumVariant {
                name: "None".to_string(),
                fields: Vec::new(),
                discriminant: None,
            },
        ],
    });
    let main_fn = func(
        "main",
        &[],
        vec![],
        i32_ty(),
        vec![
            Node::LetStmt(LetStmt {
                name: "x".to_string(),
                ty: Some(Box::new(ud("Option", vec![i32_ty()]))),
                value: Box::new(Node::EnumLiteral(EnumLiteral {
                    enum_name: "Option".to_string(),
                    variant_name: "Some".to_string(),
                    fields: vec![EnumLiteralField {
                        name: "value".to_string(),
                        value: Box::new(int_lit(5)),
                    }],
                })),
            }),
            ret(int_lit(0)),
        ],
    );
    let mut root = program(vec![option, main_fn]);
    monomorphize_program(&mut root).expect("monomorphize must succeed");
    let main_fn = find_func(&root, "main").expect("main stays");
    let Node::LetStmt(l) = &main_fn.body[0] else {
        panic!("first statement must be the let")
    };
    assert_eq!(l.ty.as_deref(), Some(&ud("Option_i32", vec![])));
    let Node::EnumLiteral(lit) = &*l.value else {
        panic!("value must stay an enum literal")
    };
    assert_eq!(lit.enum_name, "Option_i32");
}
