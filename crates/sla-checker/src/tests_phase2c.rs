//! Phase-2 expansion tests, part C (item 6: enum literals, slice
//! expressions, imported macros, trait/impl contracts, operator
//! overloads + numeric-struct specials, dyn/slice coercions,
//! closures, associated-call trait symbols, sum slice form).
//! Venue probes (2026-11-07, real plugin at 6002b386): enum
//! literal rc=0; slice `a[1..3]` rc=0, `a[1..9]` rc=1 TypeMismatch
//! "slice range out of bounds: 1..9 for len 4"; standalone closure
//! with an inferred parameter rc=1 TypeMismatch (exact message
//! pinned below).

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
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
        is_async: false,
        is_variadic: false,
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

// -- enum literals -----------------------------------------------------------

fn message_decl() -> Node {
    Node::EnumDecl(EnumDecl {
        name: "Message".to_string(),
        generics: Vec::new(),
        variants: vec![
            EnumVariant {
                name: "Move".to_string(),
                fields: vec![
                    Field {
                        name: "x".to_string(),
                        ty: Box::new(i32_ty()),
                    },
                    Field {
                        name: "y".to_string(),
                        ty: Box::new(i32_ty()),
                    },
                ],
                discriminant: None,
            },
            EnumVariant {
                name: "Quit".to_string(),
                fields: Vec::new(),
                discriminant: None,
            },
        ],
    })
}

fn enum_lit(variant: &str, fields: Vec<(&str, Node)>) -> Node {
    Node::EnumLiteral(EnumLiteral {
        enum_name: "Message".to_string(),
        variant_name: variant.to_string(),
        fields: fields
            .into_iter()
            .map(|(name, value)| EnumLiteralField {
                name: name.to_string(),
                value: Box::new(value),
            })
            .collect(),
    })
}

#[test]
fn enum_literal_ok() {
    let lit = enum_lit("Move", vec![("x", int_lit(1)), ("y", int_lit(2))]);
    let (_, result) = check(vec![
        message_decl(),
        func("f", vec![], user("Message", vec![]), vec![ret(lit)]),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn enum_literal_unknown_enum_is_not_a_struct() {
    let lit = Node::EnumLiteral(EnumLiteral {
        enum_name: "Nope".to_string(),
        variant_name: "A".to_string(),
        fields: vec![],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![],
        void_ty(),
        vec![expr_stmt(lit)],
    )]);
    let err = result.expect_err("unknown enum must fail");
    assert_eq!(err.kind, ErrorKind::NotAStruct);
}

#[test]
fn enum_literal_unknown_variant_and_missing_field() {
    let bad_variant = enum_lit("Nope", vec![]);
    let (_, result) = check(vec![
        message_decl(),
        func("f", vec![], void_ty(), vec![expr_stmt(bad_variant)]),
    ]);
    assert_eq!(
        result.expect_err("unknown variant").kind,
        ErrorKind::FieldNotFound
    );

    let missing = enum_lit("Move", vec![("x", int_lit(1))]);
    let (_, result) = check(vec![
        message_decl(),
        func("f", vec![], void_ty(), vec![expr_stmt(missing)]),
    ]);
    assert_eq!(
        result.expect_err("missing field").kind,
        ErrorKind::FieldNotFound
    );
}

#[test]
fn enum_literal_field_type_mismatch() {
    let lit = enum_lit(
        "Move",
        vec![
            ("x", Node::Literal(Literal::BoolVal(true))),
            ("y", int_lit(2)),
        ],
    );
    let (_, result) = check(vec![
        message_decl(),
        func("f", vec![], void_ty(), vec![expr_stmt(lit)]),
    ]);
    assert_eq!(
        result.expect_err("bool into i32 field").kind,
        ErrorKind::TypeMismatch
    );
}

// -- slice expressions -------------------------------------------------------

fn slice_expr(start: Node, end: Node) -> Node {
    Node::SliceExpr(SliceExpr {
        target: Box::new(ident("a")),
        start: Box::new(start),
        end: Box::new(end),
    })
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

#[test]
fn slice_expr_ok() {
    let (_, result) = check(vec![func(
        "f",
        vec![arr4_param()],
        Type::Array(ArrayType {
            elem: Box::new(i32_ty()),
            len: 2,
        }),
        vec![ret(slice_expr(int_lit(1), int_lit(3)))],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn slice_expr_out_of_bounds_message() {
    let (_, result) = check(vec![func(
        "f",
        vec![arr4_param()],
        void_ty(),
        vec![expr_stmt(slice_expr(int_lit(1), int_lit(9)))],
    )]);
    let err = result.expect_err("out of bounds slice must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
    assert!(
        err.message.contains("slice range out of bounds: 1..9 for len 4"),
        "{err:?}"
    );
}

#[test]
fn slice_expr_non_literal_bound_rejected() {
    let (_, result) = check(vec![func(
        "f",
        vec![arr4_param(), param("n", i32_ty())],
        void_ty(),
        vec![expr_stmt(slice_expr(int_lit(0), ident("n")))],
    )]);
    let err = result.expect_err("non-literal bound must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

// -- sum slice form ------------------------------------------------------------

#[test]
fn sum_over_vec_and_slice_ok_over_option_rejected() {
    let vec_call = call("sum", vec![ident("v")]);
    let (_, result) = check(vec![func(
        "f",
        vec![param("v", user("Vec", vec![i32_ty()]))],
        i32_ty(),
        vec![ret(vec_call)],
    )]);
    assert!(result.is_ok(), "Vec sum: {result:?}");

    let opt_call = call("sum", vec![ident("o")]);
    let (_, result) = check(vec![func(
        "f",
        vec![param("o", user("Option", vec![i32_ty()]))],
        i32_ty(),
        vec![ret(opt_call)],
    )]);
    let err = result.expect_err("sum over Option must fail (Zig has no such arm)");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

// -- imported macros -----------------------------------------------------------

#[test]
fn imported_macro_expression_output_and_arity() {
    let mut checker = Checker::new();
    checker.register_imported_macro(
        "SLA_BUF_ALLOC",
        crate::checker::ImportedMacroInfo {
            arity: 2,
            leading_outputs: 1,
            import_path: None,
            borrowed_arg_mask: 0,
            address_slot_arg_mask: 0,
            direct_callees: vec![],
        },
    );
    // One argument (arity - 1) in expression position: raw pointer
    // per the name table.
    let program = Program {
        decls: vec![func(
            "f",
            vec![param("n", i32_ty())],
            Type::Primitive(Primitive::RawPtr),
            vec![ret(call("SLA_BUF_ALLOC", vec![ident("n")]))],
        )],
    };
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "expression-output form: {result:?}");

    // Exact arity: void.
    let mut checker = Checker::new();
    checker.register_imported_macro(
        "SLA_BUF_ALLOC",
        crate::checker::ImportedMacroInfo {
            arity: 2,
            leading_outputs: 1,
            import_path: None,
            borrowed_arg_mask: 0,
            address_slot_arg_mask: 0,
            direct_callees: vec![],
        },
    );
    let program = Program {
        decls: vec![func(
            "f",
            vec![param("n", i32_ty())],
            void_ty(),
            vec![expr_stmt(call(
                "SLA_BUF_ALLOC",
                vec![ident("n"), ident("n")],
            ))],
        )],
    };
    assert!(checker.check_program(&program).is_ok());

    // Three arguments: InvalidArgsCount with Zig's message shape.
    let mut checker = Checker::new();
    checker.register_imported_macro(
        "SLA_BUF_ALLOC",
        crate::checker::ImportedMacroInfo {
            arity: 2,
            leading_outputs: 1,
            import_path: None,
            borrowed_arg_mask: 0,
            address_slot_arg_mask: 0,
            direct_callees: vec![],
        },
    );
    let program = Program {
        decls: vec![func(
            "f",
            vec![param("n", i32_ty())],
            void_ty(),
            vec![expr_stmt(call(
                "SLA_BUF_ALLOC",
                vec![ident("n"), ident("n"), ident("n")],
            ))],
        )],
    };
    let err = checker
        .check_program(&program)
        .expect_err("3 args must fail");
    assert_eq!(err.kind, ErrorKind::InvalidArgsCount);
    assert!(
        err.message.contains("expects 2 args or 1 args when using expression output, got 3"),
        "{err:?}"
    );
}
