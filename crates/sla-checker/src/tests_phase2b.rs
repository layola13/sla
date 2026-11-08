//! Phase-2 expansion tests, part B (item 3: let-else, destructure,
//! while-let). Split out of tests_phase2.rs to respect the 700-line
//! per-file rule; builders are intentionally duplicated.

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

fn string_ty() -> Type {
    Type::UserDefined(UserDefined {
        name: "String".to_string(),
        generics: Vec::new(),
    })
}

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn void_ty() -> Type {
    Type::Primitive(Primitive::VoidType)
}

fn option_ty(inner: Type) -> Type {
    Type::UserDefined(UserDefined {
        name: "Option".to_string(),
        generics: vec![inner],
    })
}

fn some_pattern(binding: &str) -> EnumPattern {
    EnumPattern {
        enum_name: "Option".to_string(),
        variant_name: "Some".to_string(),
        bindings: vec![binding.to_string()],
    }
}

fn check(decls: Vec<Node>) -> (Checker, Result<(), crate::diag::Diagnostic>) {
    let mut checker = Checker::new();
    let program = Program { decls };
    let result = checker.check_program(&program);
    (checker, result)
}
// -- item 3: let-else / destructure / while-let ----------------------------

#[test]
fn let_else_ok_binds_value() {
    // Venue probe p_letelse_ok: rc=0.
    let stmt = Node::LetElseStmt(LetElseStmt {
        pattern: some_pattern("v"),
        value: Box::new(ident("o")),
        else_block: vec![ret(int_lit(0))],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("o", option_ty(i32_ty()))],
        i32_ty(),
        vec![stmt, ret(ident("v"))],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn let_else_non_diverging_rejected() {
    // Venue probe p_letelse_bad: rc=1, TypeMismatch
    // "let else block must diverge".
    let stmt = Node::LetElseStmt(LetElseStmt {
        pattern: some_pattern("v"),
        value: Box::new(ident("o")),
        else_block: vec![let_stmt("x", int_lit(1))],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("o", option_ty(i32_ty()))],
        i32_ty(),
        vec![stmt, ret(ident("v"))],
    )]);
    let err = result.expect_err("non-diverging let-else must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
    assert!(err.message.contains("must diverge"), "{err:?}");
}

#[test]
fn destructure_tuple_binds_and_consumes_root() {
    let destr = Node::LetDestructureStmt(LetDestructureStmt {
        names: vec!["a".to_string(), "b".to_string()],
        value: Box::new(ident("t")),
        rest_name: None,
        rest_alias: None,
        is_slice: false,
    });
    let tuple_ty = Type::Tuple(TupleType {
        elems: vec![string_ty(), i32_ty()],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("t", tuple_ty)],
        i32_ty(),
        vec![destr, let_stmt("c", ident("t")), expr_stmt(ident("b"))],
    )]);
    let err = result.expect_err("use of destructured root must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn destructure_copy_tuple_still_consumes_root() {
    // Pins the Zig behavior registered as FB-010: the destructure
    // consumption site (type_checker.zig:2913) has NO Copy
    // exemption, so even a (i32, i32) tuple variable is consumed
    // (venue probe p_destr_copy: rc=1 UseAfterMove).
    let destr = Node::LetDestructureStmt(LetDestructureStmt {
        names: vec!["x".to_string(), "y".to_string()],
        value: Box::new(ident("t")),
        rest_name: None,
        rest_alias: None,
        is_slice: false,
    });
    let tuple_ty = Type::Tuple(TupleType {
        elems: vec![i32_ty(), i32_ty()],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("t", tuple_ty)],
        i32_ty(),
        vec![
            destr,
            let_stmt("c", ident("t")),
            expr_stmt(Node::BinaryExpr(BinaryExpr {
                op: BinaryOp::Add,
                left: Box::new(ident("x")),
                right: Box::new(ident("y")),
            })),
        ],
    )]);
    let err = result.expect_err("Copy tuple root must still be consumed");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn destructure_arity_mismatch_rejected() {
    let destr = Node::LetDestructureStmt(LetDestructureStmt {
        names: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        value: Box::new(ident("t")),
        rest_name: None,
        rest_alias: None,
        is_slice: false,
    });
    let tuple_ty = Type::Tuple(TupleType {
        elems: vec![i32_ty(), i32_ty()],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("t", tuple_ty)],
        void_ty(),
        vec![destr],
    )]);
    let err = result.expect_err("arity mismatch must fail");
    assert_eq!(err.kind, ErrorKind::InvalidArgsCount);
}

#[test]
fn destructure_non_tuple_rejected() {
    let destr = Node::LetDestructureStmt(LetDestructureStmt {
        names: vec!["a".to_string()],
        value: Box::new(ident("t")),
        rest_name: None,
        rest_alias: None,
        is_slice: false,
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("t", i32_ty())],
        void_ty(),
        vec![destr],
    )]);
    let err = result.expect_err("non-tuple destructure must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn destructure_slice_with_rest_binds() {
    let destr = Node::LetDestructureStmt(LetDestructureStmt {
        names: vec!["first".to_string()],
        value: Box::new(ident("a")),
        rest_name: Some("rest".to_string()),
        rest_alias: None,
        is_slice: true,
    });
    let arr_ty = Type::Array(ArrayType {
        elem: Box::new(i32_ty()),
        len: 4,
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("a", arr_ty)],
        i32_ty(),
        vec![destr, expr_stmt(ident("first"))],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn destructure_slice_without_rest_rejected() {
    let destr = Node::LetDestructureStmt(LetDestructureStmt {
        names: vec!["first".to_string()],
        value: Box::new(ident("a")),
        rest_name: None,
        rest_alias: None,
        is_slice: true,
    });
    let arr_ty = Type::Array(ArrayType {
        elem: Box::new(i32_ty()),
        len: 4,
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("a", arr_ty)],
        void_ty(),
        vec![destr],
    )]);
    let err = result.expect_err("slice destructure without rest must fail");
    assert_eq!(err.kind, ErrorKind::InvalidArgsCount);
}

#[test]
fn while_let_binds_payload() {
    // Venue probe p_whilelet: rc=0.
    let stmt = Node::WhileStmt(WhileStmt {
        cond: Box::new(ident("o")),
        let_pattern: Some(some_pattern("v")),
        body: vec![ret(ident("v"))],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("o", option_ty(i32_ty()))],
        i32_ty(),
        vec![stmt, ret(int_lit(0))],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn while_let_pattern_mismatch_rejected() {
    let stmt = Node::WhileStmt(WhileStmt {
        cond: Box::new(ident("o")),
        let_pattern: Some(EnumPattern {
            enum_name: "Result".to_string(),
            variant_name: "Ok".to_string(),
            bindings: vec!["v".to_string()],
        }),
        body: vec![],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("o", option_ty(i32_ty()))],
        void_ty(),
        vec![stmt],
    )]);
    let err = result.expect_err("Result pattern on Option must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

// -- item 4: generics (checker-level) --------------------------------------

#[test]
fn generic_func_ref_rejected_compile_error() {
    // Zig type_checker.zig:3209 — the arm is a bare
    // `return TypeError.CompileError` (FB-012).
    let gref = Node::GenericFuncRef(GenericFuncRef {
        func_name: "id".to_string(),
        generics: vec![],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![],
        i32_ty(),
        vec![expr_stmt(gref), ret(int_lit(0))],
    )]);
    let err = result.expect_err("generic func ref must be rejected");
    assert_eq!(err.kind, ErrorKind::CompileError);
}

#[test]
fn generic_function_checked_against_unsubstituted_signature() {
    // D-C11: instantiation is the monomorphizer's job; the checker
    // sees the generic body with `T` as an opaque user type and
    // checks calls against the RAW signature, so `id(5)` mismatches
    // `T` against i64 at checker level (the real pipeline never
    // feeds this shape to the checker — mono specializes first;
    // venue probe p_generic passes through the full pipeline).
    let mut id_fn = match func("id", vec![param("x", generic_t())], generic_t(), vec![
        ret(ident("x")),
    ]) {
        Node::FuncDecl(mut f) => {
            f.generics = vec!["T".to_string()];
            Node::FuncDecl(f)
        }
        _ => unreachable!(),
    };
    let _ = &mut id_fn;
    let (_, result) = check(vec![id_fn]);
    assert!(result.is_ok(), "generic body must check: {result:?}");

    let call = Node::CallExpr(CallExpr {
        func_name: "id".to_string(),
        args: vec![int_lit(5)],
        associated_target: None,
        callee: None,
        generics: vec![i32_ty()],
    });
    let mut id_fn2 = match func("id", vec![param("x", generic_t())], generic_t(), vec![
        ret(ident("x")),
    ]) {
        Node::FuncDecl(mut f) => {
            f.generics = vec!["T".to_string()];
            Node::FuncDecl(f)
        }
        _ => unreachable!(),
    };
    let _ = &mut id_fn2;
    let (_, result) = check(vec![
        id_fn2,
        func("g", vec![], i32_ty(), vec![expr_stmt(call), ret(int_lit(0))]),
    ]);
    let err = result.expect_err("raw-signature call must mismatch");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

fn generic_t() -> Type {
    Type::UserDefined(UserDefined {
        name: "T".to_string(),
        generics: Vec::new(),
    })
}

// -- item 5: side tables ----------------------------------------------------

fn result_ty() -> Type {
    Type::UserDefined(UserDefined {
        name: "Result".to_string(),
        generics: vec![i32_ty(), i32_ty()],
    })
}

#[test]
fn await_records_cleanup_and_consumes_future() {
    // Venue probe p_await_consume: rc=1 (UseAfterMove) on a second
    // await of the same future. The cleanup snapshot excludes the
    // awaited binding (escape rule) and includes other actives.
    let await_expr = Node::AwaitExpr(AwaitExpr {
        expr: Box::new(ident("fut")),
    });
    let (checker, result) = check(vec![func(
        "f",
        vec![
            param("fut", Type::Future(Box::new(i32_ty()))),
            param("s", string_ty()),
        ],
        i32_ty(),
        vec![let_stmt("a", await_expr), ret(ident("a"))],
    )]);
    assert!(result.is_ok(), "{result:?}");
    let lists: Vec<&Vec<String>> = checker.await_cleanups.values().collect();
    assert_eq!(lists.len(), 1, "{lists:?}");
    assert!(lists[0].iter().any(|n| n == "s"), "{lists:?}");
    assert!(!lists[0].iter().any(|n| n == "fut"), "{lists:?}");
}

#[test]
fn await_twice_is_use_after_move() {
    let body = vec![
        let_stmt(
            "a",
            Node::AwaitExpr(AwaitExpr {
                expr: Box::new(ident("fut")),
            }),
        ),
        let_stmt(
            "b",
            Node::AwaitExpr(AwaitExpr {
                expr: Box::new(ident("fut")),
            }),
        ),
        ret(ident("a")),
    ];
    let (_, result) = check(vec![func(
        "f",
        vec![param("fut", Type::Future(Box::new(i32_ty())))],
        i32_ty(),
        body,
    )]);
    let err = result.expect_err("second await must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn await_non_future_rejected() {
    let await_expr = Node::AwaitExpr(AwaitExpr {
        expr: Box::new(ident("x")),
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("x", i32_ty())],
        i32_ty(),
        vec![expr_stmt(await_expr), ret(int_lit(0))],
    )]);
    let err = result.expect_err("await on i32 must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
    assert!(err.message.contains("await requires future"), "{err:?}");
}

#[test]
fn try_result_unwraps_consumes_and_records_cleanup() {
    let try_expr = Node::TryExpr(TryExpr {
        expr: Box::new(ident("r")),
    });
    let (checker, result) = check(vec![func(
        "f",
        vec![param("r", result_ty()), param("s", string_ty())],
        i32_ty(),
        vec![let_stmt("v", try_expr), ret(ident("v"))],
    )]);
    assert!(result.is_ok(), "{result:?}");
    let lists: Vec<&Vec<String>> = checker.cleanups.values().collect();
    assert!(
        lists.iter().any(|l| l.iter().any(|n| n == "s")),
        "try cleanup must include s: {lists:?}"
    );
    assert!(
        !lists.iter().any(|l| l.iter().any(|n| n == "r")),
        "try cleanup must exclude the propagated value: {lists:?}"
    );
}

#[test]
fn try_consumes_source() {
    let body = vec![
        let_stmt(
            "v",
            Node::TryExpr(TryExpr {
                expr: Box::new(ident("r")),
            }),
        ),
        let_stmt("w", ident("r")),
        ret(ident("v")),
    ];
    let (_, result) = check(vec![func(
        "f",
        vec![param("r", result_ty())],
        i32_ty(),
        body,
    )]);
    let err = result.expect_err("reuse after ? must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn try_on_plain_value_is_not_a_struct() {
    let try_expr = Node::TryExpr(TryExpr {
        expr: Box::new(ident("x")),
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("x", i32_ty())],
        i32_ty(),
        vec![expr_stmt(try_expr), ret(int_lit(0))],
    )]);
    let err = result.expect_err("? on i32 must fail");
    assert_eq!(err.kind, ErrorKind::NotAStruct);
}

#[test]
fn macro_try_call_records_cleanup_and_consumes() {
    // Macro `m(x) { x?; }`: the parameter is a try source, so the
    // call records a macro_call_try_cleanups entry covering every
    // OTHER active binding, and the propagated argument is
    // consumed (try counts as an unconditional use).
    let mac = Node::MacroDecl(MacroDecl {
        name: "m".to_string(),
        params: vec!["x".to_string()],
        param_tys: vec![None],
        body: vec![expr_stmt(Node::TryExpr(TryExpr {
            expr: Box::new(ident("x")),
        }))],
    });
    let call = Node::CallExpr(CallExpr {
        func_name: "m".to_string(),
        args: vec![ident("r")],
        associated_target: None,
        callee: None,
        generics: vec![],
    });
    let (checker, result) = check(vec![
        mac,
        func(
            "f",
            vec![param("r", result_ty()), param("s", string_ty())],
            void_ty(),
            vec![expr_stmt(call)],
        ),
    ]);
    assert!(result.is_ok(), "{result:?}");
    let lists: Vec<&Vec<String>> = checker.macro_call_try_cleanups.values().collect();
    assert_eq!(lists.len(), 1, "{lists:?}");
    assert!(lists[0].iter().any(|n| n == "s"), "{lists:?}");
    assert!(!lists[0].iter().any(|n| n == "r"), "{lists:?}");
}
