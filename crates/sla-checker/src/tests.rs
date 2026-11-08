//! Slice tests over hand-built `sla_ast` trees. Each test names the
//! Zig behavior it pins; the shadowing test additionally documents a
//! case where the two models JUDGE DIFFERENTLY (D-C1) — the expected
//! value there is the Rust model's verdict, with the Zig-side
//! mechanism proven from source in docs/divergences/checker.md.

use crate::checker::{Checker, ExternSig, FuncSig, ParamSig};
use crate::diag::ErrorKind;
use sla_ast::decls::*;
use sla_ast::exprs::*;
use sla_ast::node::Node;
use sla_ast::stmts::*;
use sla_ast::types::*;

// -- builders ---------------------------------------------------------

fn int_lit(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn bool_lit(v: bool) -> Node {
    Node::Literal(Literal::BoolVal(v))
}

fn str_lit(v: &str) -> Node {
    Node::Literal(Literal::StringVal(v.to_string()))
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn bin(op: BinaryOp, left: Node, right: Node) -> Node {
    Node::BinaryExpr(BinaryExpr {
        op,
        left: Box::new(left),
        right: Box::new(right),
    })
}

fn expr_stmt(e: Node) -> Node {
    Node::ExprStmt(Box::new(e))
}

fn let_stmt(name: &str, ty: Option<Type>, value: Node) -> Node {
    Node::LetStmt(LetStmt {
        name: name.to_string(),
        ty: ty.map(Box::new),
        value: Box::new(value),
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

fn func(name: &str, params: Vec<Param>, ret: Type, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params,
        ret_ty: Box::new(ret),
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

fn bool_ty() -> Type {
    Type::Primitive(Primitive::Boolean)
}

fn void_ty() -> Type {
    Type::Primitive(Primitive::VoidType)
}

fn check(decls: Vec<Node>) -> (Checker, Result<(), crate::diag::Diagnostic>) {
    let mut checker = Checker::new();
    let program = Program { decls };
    let result = checker.check_program(&program);
    (checker, result)
}

// -- typing basics ------------------------------------------------------

#[test]
fn arith_function_passes() {
    // fn add(a: i32, b: i32) -> i32 { a + b }   (fixture 07 shape)
    let (_, result) = check(vec![func(
        "add",
        vec![param("a", i32_ty()), param("b", i32_ty())],
        i32_ty(),
        vec![expr_stmt(bin(BinaryOp::Add, ident("a"), ident("b")))],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn recursive_factorial_passes() {
    // fn fact(n: i32) -> i32 { if n <= 1 { 1 } else { n * fact(n - 1) } }
    let if_expr = Node::IfExpr(IfExpr {
        cond: Box::new(bin(BinaryOp::Le, ident("n"), int_lit(1))),
        let_chain: None,
        then_block: vec![expr_stmt(int_lit(1))],
        else_block: Some(vec![expr_stmt(bin(
            BinaryOp::Mul,
            ident("n"),
            Node::CallExpr(CallExpr {
                func_name: "fact".to_string(),
                associated_target: None,
                generics: Vec::new(),
                args: vec![bin(BinaryOp::Sub, ident("n"), int_lit(1))],
                callee: None,
            }),
        ))]),
    });
    let (_, result) = check(vec![func(
        "fact",
        vec![param("n", i32_ty())],
        i32_ty(),
        vec![expr_stmt(if_expr)],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn width_insensitive_let_passes() {
    // Pins D-C8: the int literal types as i64, the declared type is
    // i32, and Zig's typesEqual accepts the width difference.
    let (_, result) = check(vec![func(
        "f",
        vec![],
        i32_ty(),
        vec![
            let_stmt("x", Some(i32_ty()), int_lit(0)),
            expr_stmt(ident("x")),
        ],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

// -- ownership state machine -------------------------------------------

#[test]
fn use_after_move_via_let() {
    // fn f(s: String) { let a = s; let b = s; }
    let (_, result) = check(vec![func(
        "f",
        vec![param("s", string_ty())],
        void_ty(),
        vec![
            let_stmt("a", None, ident("s")),
            let_stmt("b", None, ident("s")),
        ],
    )]);
    let err = result.expect_err("second let must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn loop_conditional_consume_rejected() {
    // fn f(s: String) { while true { let a = s; } }
    // The DEFECT-A family gate: consuming a pre-loop binding inside a
    // non-terminating loop body is a CompileError whose NAME exists
    // only in the message text in Zig; here kind + marker.
    let (_, result) = check(vec![func(
        "f",
        vec![param("s", string_ty())],
        void_ty(),
        vec![Node::WhileStmt(WhileStmt {
            cond: Box::new(bool_lit(true)),
            let_pattern: None,
            body: vec![let_stmt("a", None, ident("s"))],
        })],
    )]);
    let err = result.expect_err("loop consume must fail");
    assert_eq!(err.kind, ErrorKind::CompileError);
    assert_eq!(err.marker, Some("LoopConditionalConsume"));
}

#[test]
fn if_merge_consumes_single_branch_binding() {
    // fn f(s: String, flag: bool) { if flag { let a = s; } else { 0; }
    //   let b = s; }
    // s is consumed on the then path only: after the merge it is
    // consumed, and the else path (where it stayed active) earns a
    // phi cleanup.
    let if_expr = Node::IfExpr(IfExpr {
        cond: Box::new(ident("flag")),
        let_chain: None,
        then_block: vec![let_stmt("a", None, ident("s"))],
        else_block: Some(vec![expr_stmt(int_lit(0))]),
    });
    let (checker, result) = check(vec![func(
        "f",
        vec![param("s", string_ty()), param("flag", bool_ty())],
        void_ty(),
        vec![expr_stmt(if_expr), let_stmt("b", None, ident("s"))],
    )]);
    let err = result.expect_err("post-merge use must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
    let phi_names: Vec<&String> = checker
        .phi_cleanups
        .values()
        .flat_map(|v| v.iter())
        .collect();
    assert!(
        phi_names.iter().any(|n| n.as_str() == "s"),
        "phi cleanup for s expected, got {phi_names:?}"
    );
}

#[test]
fn var_assigned_only_in_loop_is_uninit_after() {
    // fn f(flag: bool) { var x: i32; while flag { x = 1; } let y = x; }
    // Loop bodies are checked once, so an assignment inside does not
    // prove initialization after the loop (restore-uninitialized).
    let (_, result) = check(vec![func(
        "f",
        vec![param("flag", bool_ty())],
        void_ty(),
        vec![
            Node::VarStmt(VarStmt {
                name: "x".to_string(),
                ty: Box::new(i32_ty()),
            }),
            Node::WhileStmt(WhileStmt {
                cond: Box::new(ident("flag")),
                let_pattern: None,
                body: vec![Node::AssignStmt(AssignStmt {
                    target: Box::new(ident("x")),
                    value: Box::new(int_lit(1)),
                })],
            }),
            let_stmt("y", None, ident("x")),
        ],
    )]);
    let err = result.expect_err("read of possibly-uninit x must fail");
    assert_eq!(err.kind, ErrorKind::UseBeforeInit);
}

#[test]
fn shadowing_snapshot_is_per_binding() {
    // D-C1 demonstrator. Outer `s` (param) is shadowed inside a block
    // by a second `s`; the INNER s is consumed on one if-branch.
    // Zig's name-keyed flat snapshot holds the OUTER binding's state
    // under the shared key "s", so its merge sees Active == Active
    // and the inner consumption is lost — `let b = s` would pass.
    // The identity-keyed model merges the inner binding to Consumed
    // and rejects the use.
    let if_expr = Node::IfExpr(IfExpr {
        cond: Box::new(ident("flag")),
        let_chain: None,
        then_block: vec![let_stmt("a", None, ident("s"))],
        else_block: Some(vec![]),
    });
    let inner_block = Node::BlockStmt(BlockStmt {
        body: vec![
            let_stmt("s", None, ident("t")),
            expr_stmt(if_expr),
            let_stmt("b", None, ident("s")),
        ],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![
            param("s", string_ty()),
            param("t", string_ty()),
            param("flag", bool_ty()),
        ],
        void_ty(),
        vec![inner_block],
    )]);
    let err = result.expect_err("inner s was consumed on the then path");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

// -- plain-call argument consumption (C1, ex D-C7) ---------------------

fn plain_call(name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: None,
        generics: Vec::new(),
        args,
        callee: None,
    })
}

fn h_decl() -> Node {
    func("h", vec![param("s", string_ty())], void_ty(), vec![])
}

#[test]
fn plain_call_consumes_bare_read() {
    // C1 (consult-dc7, six-house unanimous; supersedes the D-C7 pin):
    // a plain by-value call argument IS consumed at checker level,
    // so the bare read after the call — `h(s); let b = s;` — is a
    // UseAfterMove (REG-BR-6 text), exactly like a second call.
    let (_, result) = check(vec![
        h_decl(),
        func(
            "f",
            vec![param("s", string_ty())],
            void_ty(),
            vec![
                expr_stmt(plain_call("h", vec![ident("s")])),
                let_stmt("b", None, ident("s")),
            ],
        ),
    ]);
    let err = result.expect_err("bare read after consuming call must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn plain_call_consumes_second_call() {
    // The corpus-forced shape (REG-BR-6): the same owned String
    // passed by value to a second call after the first moved it.
    let (_, result) = check(vec![
        h_decl(),
        func(
            "f",
            vec![param("s", string_ty())],
            void_ty(),
            vec![
                expr_stmt(plain_call("h", vec![ident("s")])),
                expr_stmt(plain_call("h", vec![ident("s")])),
            ],
        ),
    ]);
    let err = result.expect_err("second call after consuming call must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn plain_call_consumes_compare_read() {
    // Comparison read after the call is still a use of the consumed
    // binding (probe matrix S2 in the R-6 line: Err under C1).
    let (_, result) = check(vec![
        h_decl(),
        func(
            "f",
            vec![param("s", string_ty())],
            void_ty(),
            vec![
                expr_stmt(plain_call("h", vec![ident("s")])),
                expr_stmt(bin(BinaryOp::Eq, ident("s"), str_lit("x"))),
            ],
        ),
    ]);
    let err = result.expect_err("compare read after consuming call must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn plain_call_consumes_return_use() {
    // `h(s); return s;` — returning the consumed binding fails too.
    let (_, result) = check(vec![
        h_decl(),
        func(
            "f",
            vec![param("s", string_ty())],
            string_ty(),
            vec![
                expr_stmt(plain_call("h", vec![ident("s")])),
                Node::ReturnStmt(ReturnStmt {
                    value: Some(Box::new(ident("s"))),
                }),
            ],
        ),
    ]);
    let err = result.expect_err("return of consumed binding must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn read_before_call_then_error_at_call_site() {
    // A read BEFORE the call is legal; the error lands on the call
    // argument itself when the binding was already moved by the
    // preceding let (V3 shape: error at the call site, proving the
    // consumption machine fires uniformly per use site).
    let (_, result) = check(vec![
        h_decl(),
        func(
            "f",
            vec![param("s", string_ty())],
            void_ty(),
            vec![
                let_stmt("b", None, ident("s")),
                expr_stmt(plain_call("h", vec![ident("s")])),
            ],
        ),
    ]);
    let err = result.expect_err("call of let-moved binding must fail at the call");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
    // The firing path is the call-argument identifier check
    // (check_call_arg_consumption -> check_identifier), which reports
    // the use point of the already-consumed binding; the error
    // materializes only because the call reaches that check.
    assert!(
        err.message.contains("identifier `s` was already consumed"),
        "unexpected error text: {}",
        err.message
    );
}

#[test]
fn while_sized_int_condition_rejected() {
    // Zig's while arm accepts exactly boolean / legacy integer
    // (type_checker.zig ~3078-3085): a bare i32 condition is a
    // TypeMismatch there, so it is here too.
    let (_, result) = check(vec![func(
        "f",
        vec![param("x", i32_ty())],
        void_ty(),
        vec![Node::WhileStmt(WhileStmt {
            cond: Box::new(ident("x")),
            let_pattern: None,
            body: vec![],
        })],
    )]);
    let err = result.expect_err("i32 while condition must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn struct_literal_missing_and_duplicate_fields() {
    let point = Node::StructDecl(StructDecl {
        name: "Point".to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
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
        is_union: false,
        is_opaque: false,
    });
    let point_ty = Type::UserDefined(UserDefined {
        name: "Point".to_string(),
        generics: Vec::new(),
    });
    let lit = |fields: Vec<(&str, i64)>| {
        Node::StructLiteral(StructLiteral {
            ty: Box::new(point_ty.clone()),
            fields: fields
                .into_iter()
                .map(|(name, v)| StructLiteralField {
                    name: name.to_string(),
                    value: Box::new(int_lit(v)),
                })
                .collect(),
            update_expr: None,
        })
    };
    // Missing y -> FieldNotFound.
    let (_, result) = check(vec![
        point.clone(),
        func(
            "f",
            vec![],
            point_ty.clone(),
            vec![expr_stmt(lit(vec![("x", 1)]))],
        ),
    ]);
    let err = result.expect_err("missing field must fail");
    assert_eq!(err.kind, ErrorKind::FieldNotFound);
    // Duplicate x -> CompileError.
    let (_, result) = check(vec![
        point,
        func(
            "f",
            vec![],
            point_ty.clone(),
            vec![expr_stmt(lit(vec![("x", 1), ("x", 2), ("y", 3)]))],
        ),
    ]);
    let err = result.expect_err("duplicate field must fail");
    assert_eq!(err.kind, ErrorKind::CompileError);
}

// -- macros / extern ------------------------------------------------------

#[test]
fn macro_conditional_consume_rejected() {
    // macro m(x) { if flag { ^x } } — body analysis says the param is
    // consumed only on some paths: CompileError MacroConditionalConsume.
    let mac = Node::MacroDecl(MacroDecl {
        name: "m".to_string(),
        params: vec!["x".to_string()],
        param_tys: vec![None],
        body: vec![expr_stmt(Node::IfExpr(IfExpr {
            cond: Box::new(ident("flag")),
            let_chain: None,
            then_block: vec![expr_stmt(Node::MoveExpr(MoveExpr {
                expr: Box::new(ident("x")),
            }))],
            else_block: None,
        }))],
    });
    let call = expr_stmt(Node::CallExpr(CallExpr {
        func_name: "m".to_string(),
        associated_target: None,
        generics: Vec::new(),
        args: vec![ident("s")],
        callee: None,
    }));
    let (_, result) = check(vec![
        mac,
        func(
            "f",
            vec![param("s", string_ty())],
            void_ty(),
            vec![call],
        ),
    ]);
    let err = result.expect_err("conditional macro consume must fail");
    assert_eq!(err.kind, ErrorKind::CompileError);
    assert_eq!(err.marker, Some("MacroConditionalConsume"));
}

#[test]
fn extern_bypass_ignores_arg_types() {
    // FFI: a String argument to an extern expecting "anything" passes
    // because the Zig arm ignores argument types entirely.
    let mut checker = Checker::new();
    checker.register_extern_func(
        "c_write",
        ExternSig {
            move_params: vec![false],
            params: Vec::new(),
            is_variadic: false,
            ret_abi: "i32".to_string(),
            fallible: false,
        },
    );
    let program = Program {
        decls: vec![func(
            "f",
            vec![param("s", string_ty())],
            i32_ty(),
            vec![expr_stmt(Node::CallExpr(CallExpr {
                func_name: "c_write".to_string(),
                associated_target: None,
                generics: Vec::new(),
                args: vec![ident("s")],
                callee: None,
            }))],
        )],
    };
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn extern_move_contract_consumes() {
    let mut checker = Checker::new();
    checker.register_extern_func(
        "c_consume",
        ExternSig {
            move_params: vec![true],
            params: Vec::new(),
            is_variadic: false,
            ret_abi: "void".to_string(),
            fallible: false,
        },
    );
    let program = Program {
        decls: vec![func(
            "f",
            vec![param("s", string_ty())],
            void_ty(),
            vec![
                expr_stmt(Node::CallExpr(CallExpr {
                    func_name: "c_consume".to_string(),
                    associated_target: None,
                    generics: Vec::new(),
                    args: vec![ident("s")],
                    callee: None,
                })),
                let_stmt("b", None, ident("s")),
            ],
        )],
    };
    let err = checker
        .check_program(&program)
        .expect_err("use after extern move must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

// -- fixtures shape -------------------------------------------------------

#[test]
fn imported_println_in_test_block() {
    // Fixture 01 shape: @test { println("..."); } with println coming
    // from the imported-signature table.
    let mut checker = Checker::new();
    checker.register_imported_signature(
        "println",
        FuncSig {
            params: vec![ParamSig {
                is_var: false,
                name: "s".to_string(),
                ty: Type::Primitive(Primitive::RawPtr),
                is_borrow: false,
                is_move: false,
            }],
            ret_ty: void_ty(),
            is_pub: true,
            ret_fallible_sugar: false,
            ret_borrow_origin: None,
            is_async: false,
            is_variadic: false,
        },
    );
    let program = Program {
        decls: vec![Node::TestDecl(TestDecl {
            name: "hello".to_string(),
            is_ignored: false,
            should_panic: false,
            body: vec![expr_stmt(Node::CallExpr(CallExpr {
                func_name: "println".to_string(),
                associated_target: None,
                generics: Vec::new(),
                args: vec![str_lit("hello")],
                callee: None,
            }))],
        })],
    };
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "{result:?}");
}

// -- pure rules -----------------------------------------------------------

#[test]
fn branch_merge_plan_truth_table() {
    use crate::rules::*;
    assert_eq!(
        plan_branch_state_merge(true, true),
        BranchStateMergeAction::RestorePre
    );
    assert_eq!(
        plan_branch_state_merge(true, false),
        BranchStateMergeAction::RestoreElse
    );
    assert_eq!(
        plan_branch_state_merge(false, true),
        BranchStateMergeAction::RestoreThen
    );
    assert_eq!(
        plan_branch_state_merge(false, false),
        BranchStateMergeAction::KeepCurrent
    );
    assert_eq!(
        intersect_live_branch_value_states(&[
            ValueState::Active,
            ValueState::Consumed
        ]),
        ValueState::Consumed
    );
    assert_eq!(
        intersect_live_branch_value_states(&[
            ValueState::Active,
            ValueState::Uninitialized
        ]),
        ValueState::Uninitialized
    );
}
