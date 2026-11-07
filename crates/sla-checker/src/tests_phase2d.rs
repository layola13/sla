//! Phase-2 expansion tests, part D (item 6 continued: trait/impl
//! contracts, operator overloads + numeric-struct specials,
//! dyn/slice coercions, closures). Split out of tests_phase2c.rs
//! for the 700-line file rule; builders intentionally duplicated.

use crate::checker::Checker;
use crate::diag::ErrorKind;
use sla_ast::decls::*;
use sla_ast::exprs::*;
use sla_ast::node::Node;
use sla_ast::stmts::*;
use sla_ast::types::*;

fn int_lit(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn expr_stmt(e: Node) -> Node {
    Node::ExprStmt(Box::new(e))
}

fn let_stmt(name: &str, value: Node) -> Node {
    Node::LetStmt(LetStmt {
        name: name.to_string(),
        ty: None,
        value: Box::new(value),
    })
}

fn let_typed(name: &str, ty: Type, value: Node) -> Node {
    Node::LetStmt(LetStmt {
        name: name.to_string(),
        ty: Some(Box::new(ty)),
        value: Box::new(value),
    })
}

fn ret(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(value)),
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
    })
}

fn call(name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: None,
        generics: vec![],
        args,
        callee: None,
    })
}

fn bin(op: BinaryOp, l: Node, r: Node) -> Node {
    Node::BinaryExpr(BinaryExpr {
        op,
        left: Box::new(l),
        right: Box::new(r),
    })
}

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn void_ty() -> Type {
    Type::Primitive(Primitive::VoidType)
}

fn bool_ty() -> Type {
    Type::Primitive(Primitive::Boolean)
}

fn string_ty() -> Type {
    Type::UserDefined(UserDefined {
        name: "String".to_string(),
        generics: Vec::new(),
    })
}

fn user(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined {
        name: name.to_string(),
        generics,
    })
}

fn check(decls: Vec<Node>) -> (Checker, Result<(), crate::diag::Diagnostic>) {
    let mut checker = Checker::new();
    let program = Program { decls };
    let result = checker.check_program(&program);
    (checker, result)
}


fn arr4_param() -> Param {
    param(
        "a",
        Type::Array(ArrayType {
            elem: Box::new(i32_ty()),
            len: 4,
        }),
    )
}

// -- trait / impl contracts ----------------------------------------------------

fn drawable_decl() -> Node {
    Node::TraitDecl(TraitDecl {
        name: "Drawable".to_string(),
        supertraits: vec![],
        methods: vec![TraitMethod {
            name: "draw".to_string(),
            params: vec![param("self", user("Circle", vec![]))],
            ret_ty: Box::new(void_ty()),
        }],
    })
}

fn circle_decl() -> Node {
    Node::StructDecl(StructDecl {
        name: "Circle".to_string(),
        derives: vec![],
        generics: vec![],
        fields: vec![Field {
            name: "r".to_string(),
            ty: Box::new(i32_ty()),
        }],
        is_union: false,
        is_opaque: false,
    })
}

fn draw_method() -> Node {
    func(
        "draw",
        vec![param("self", user("Circle", vec![]))],
        void_ty(),
        vec![],
    )
}

#[test]
fn trait_impl_ok_and_associated_call_resolves() {
    let impl_decl = Node::ImplDecl(ImplDecl {
        trait_name: Some("Drawable".to_string()),
        target_ty: Box::new(user("Circle", vec![])),
        methods: vec![draw_method()],
    });
    let call_draw = Node::CallExpr(CallExpr {
        func_name: "draw".to_string(),
        associated_target: Some("Circle".to_string()),
        generics: vec![],
        args: vec![ident("c")],
        callee: None,
    });
    let (_, result) = check(vec![
        drawable_decl(),
        circle_decl(),
        impl_decl,
        func(
            "f",
            vec![param("c", user("Circle", vec![]))],
            void_ty(),
            vec![expr_stmt(call_draw)],
        ),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn trait_impl_missing_method_rejected() {
    let impl_decl = Node::ImplDecl(ImplDecl {
        trait_name: Some("Drawable".to_string()),
        target_ty: Box::new(user("Circle", vec![])),
        methods: vec![],
    });
    let (_, result) = check(vec![drawable_decl(), circle_decl(), impl_decl]);
    let err = result.expect_err("missing trait method must fail");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
}

#[test]
fn trait_impl_extra_method_rejected() {
    let impl_decl = Node::ImplDecl(ImplDecl {
        trait_name: Some("Drawable".to_string()),
        target_ty: Box::new(user("Circle", vec![])),
        methods: vec![
            draw_method(),
            func("spin", vec![param("self", user("Circle", vec![]))], void_ty(), vec![]),
        ],
    });
    let (_, result) = check(vec![drawable_decl(), circle_decl(), impl_decl]);
    let err = result.expect_err("undeclared method must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn trait_impl_supertrait_must_be_directly_implemented() {
    let named = Node::TraitDecl(TraitDecl {
        name: "Named".to_string(),
        supertraits: vec![],
        methods: vec![],
    });
    let drawable2 = Node::TraitDecl(TraitDecl {
        name: "Drawable".to_string(),
        supertraits: vec!["Named".to_string()],
        methods: vec![TraitMethod {
            name: "draw".to_string(),
            params: vec![param("self", user("Circle", vec![]))],
            ret_ty: Box::new(void_ty()),
        }],
    });
    let impl_decl = Node::ImplDecl(ImplDecl {
        trait_name: Some("Drawable".to_string()),
        target_ty: Box::new(user("Circle", vec![])),
        methods: vec![draw_method()],
    });
    let (_, result) = check(vec![named, drawable2, circle_decl(), impl_decl]);
    let err = result.expect_err("missing supertrait impl must fail");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
}

// -- operator overloads + numeric-struct specials ------------------------------

fn point_decl(derives: Vec<String>, extra_string_field: bool) -> Node {
    let mut fields = vec![
        Field {
            name: "x".to_string(),
            ty: Box::new(i32_ty()),
        },
        Field {
            name: "y".to_string(),
            ty: Box::new(i32_ty()),
        },
    ];
    if extra_string_field {
        fields.push(Field {
            name: "label".to_string(),
            ty: Box::new(string_ty()),
        });
    }
    Node::StructDecl(StructDecl {
        name: "Point".to_string(),
        derives,
        generics: vec![],
        fields,
        is_union: false,
        is_opaque: false,
    })
}

fn point_ty() -> Type {
    user("Point", vec![])
}

#[test]
fn numeric_struct_add_and_scalar_mul() {
    let add = bin(BinaryOp::Add, ident("p"), ident("q"));
    let (_, result) = check(vec![
        point_decl(vec![], false),
        func(
            "f",
            vec![param("p", point_ty()), param("q", point_ty())],
            point_ty(),
            vec![ret(add)],
        ),
    ]);
    assert!(result.is_ok(), "struct add: {result:?}");

    let mul = bin(BinaryOp::Mul, ident("p"), int_lit(2));
    let (_, result) = check(vec![
        point_decl(vec![], false),
        func(
            "f",
            vec![param("p", point_ty())],
            point_ty(),
            vec![ret(mul)],
        ),
    ]);
    assert!(result.is_ok(), "scalar mul: {result:?}");
}

#[test]
fn numeric_struct_add_rejected_with_non_numeric_field() {
    let add = bin(BinaryOp::Add, ident("p"), ident("q"));
    let (_, result) = check(vec![
        point_decl(vec![], true),
        func(
            "f",
            vec![param("p", point_ty()), param("q", point_ty())],
            point_ty(),
            vec![ret(add)],
        ),
    ]);
    let err = result.expect_err("String field must block struct add");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn struct_eq_requires_derive() {
    let eq = bin(BinaryOp::Eq, ident("p"), ident("q"));
    let (_, result) = check(vec![
        point_decl(vec![], false),
        func(
            "f",
            vec![param("p", point_ty()), param("q", point_ty())],
            bool_ty(),
            vec![ret(eq)],
        ),
    ]);
    let err = result.expect_err("Eq without derive must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);

    let eq2 = bin(BinaryOp::Eq, ident("p"), ident("q"));
    let (_, result) = check(vec![
        point_decl(vec!["PartialEq".to_string()], false),
        func(
            "f",
            vec![param("p", point_ty()), param("q", point_ty())],
            bool_ty(),
            vec![ret(eq2)],
        ),
    ]);
    assert!(result.is_ok(), "PartialEq derive: {result:?}");
}

#[test]
fn operator_overload_resolves() {
    let mut method = match func(
        "add_impl",
        vec![param("a", point_ty()), param("b", point_ty())],
        point_ty(),
        vec![ret(ident("a"))],
    ) {
        Node::FuncDecl(mut f) => {
            f.operator = Some(BinaryOp::Add);
            Node::FuncDecl(f)
        }
        _ => unreachable!(),
    };
    let _ = &mut method;
    let overload = Node::OverloadDecl(OverloadDecl {
        target_ty: Box::new(point_ty()),
        methods: vec![method],
    });
    // With a String field the numeric-struct special would reject;
    // the registered overload must win.
    let add = bin(BinaryOp::Add, ident("p"), ident("q"));
    let (_, result) = check(vec![
        point_decl(vec![], true),
        overload,
        func(
            "f",
            vec![param("p", point_ty()), param("q", point_ty())],
            point_ty(),
            vec![ret(add)],
        ),
    ]);
    assert!(result.is_ok(), "overload add: {result:?}");
}

// -- dyn / slice coercions, const ----------------------------------------------

#[test]
fn dyn_box_coercion_recorded() {
    let impl_decl = Node::ImplDecl(ImplDecl {
        trait_name: Some("Drawable".to_string()),
        target_ty: Box::new(user("Circle", vec![])),
        methods: vec![draw_method()],
    });
    let declared = user("Box", vec![user("__dyn_Drawable", vec![])]);
    let body = vec![let_typed("d", declared, ident("b"))];
    let (checker, result) = check(vec![
        drawable_decl(),
        circle_decl(),
        impl_decl,
        func(
            "f",
            vec![param("b", user("Box", vec![user("Circle", vec![])]))],
            void_ty(),
            body,
        ),
    ]);
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(checker.dyn_box_coercions.len(), 1, "{checker:?}");
    assert!(
        checker.dyn_box_coercions.values().any(|t| t == "Drawable"),
        "{:?}",
        checker.dyn_box_coercions
    );
}

#[test]
fn slice_coercion_let_binds_without_consuming() {
    let borrow = Node::BorrowExpr(BorrowExpr {
        expr: Box::new(ident("a")),
    });
    let body = vec![
        let_typed("s", user("Slice", vec![i32_ty()]), borrow),
        expr_stmt(ident("a")),
    ];
    let (_, result) = check(vec![func(
        "f",
        vec![arr4_param()],
        void_ty(),
        body,
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn const_initializer_does_not_consume() {
    // The Zig const arm never consumes its initializer source;
    // the phase-1 mirror consumed for both let and const.
    let body = vec![
        Node::ConstStmt(ConstStmt {
            name: "C".to_string(),
            ty: None,
            value: Box::new(ident("s")),
        }),
        let_stmt("b", ident("s")),
    ];
    let (_, result) = check(vec![func(
        "f",
        vec![param("s", string_ty())],
        void_ty(),
        body,
    )]);
    assert!(result.is_ok(), "{result:?}");
}

// -- closures --------------------------------------------------------------------

#[test]
fn closure_literal_and_local_call() {
    let closure = Node::ClosureLiteral(ClosureLiteral {
        params: vec![param("x", i32_ty())],
        body: Box::new(bin(BinaryOp::Add, ident("x"), int_lit(1))),
    });
    let body = vec![
        let_stmt("f2", closure),
        ret(call("f2", vec![int_lit(41)])),
    ];
    let (_, result) = check(vec![func("f", vec![], i32_ty(), body)]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn closure_inferred_param_rejected() {
    let closure = Node::ClosureLiteral(ClosureLiteral {
        params: vec![param("x", Type::Infer)],
        body: Box::new(ident("x")),
    });
    let (_, result) = check(vec![func(
        "f",
        vec![],
        void_ty(),
        vec![let_stmt("f2", closure)],
    )]);
    let err = result.expect_err("inferred closure param must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
    assert!(
        err.message.contains("closure parameter types must be inferred from context"),
        "{err:?}"
    );
}

#[test]
fn iife_closure_call() {
    let closure = Node::ClosureLiteral(ClosureLiteral {
        params: vec![param("x", i32_ty())],
        body: Box::new(bin(BinaryOp::Mul, ident("x"), int_lit(2))),
    });
    let iife = Node::CallExpr(CallExpr {
        func_name: String::new(),
        associated_target: None,
        generics: vec![],
        args: vec![int_lit(21)],
        callee: Some(Box::new(closure)),
    });
    let (_, result) = check(vec![func("f", vec![], i32_ty(), vec![ret(iife)])]);
    assert!(result.is_ok(), "{result:?}");
}
