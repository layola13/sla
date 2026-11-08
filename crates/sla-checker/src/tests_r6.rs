//! R-6 regression tests: one minim per negative family that the
//! census found wholesale-accepted at the R-6 base (93693349).
//! Each negative test fails on the pre-fix tree (the diagnostic did
//! not exist or was unreachable) and passes once the family gate is
//! implemented; positive controls pin that the gates do not
//! over-reject. Corpus anchors are named in each test; the corpus
//! negatives themselves are judged by standalone `sla check`
//! (neg_fix7.tsv: 0 accepted of 151, identical to neg_fix2/fix5).

use crate::checker::{Checker, ExternSig};
use crate::check_overload3::{extern_sig_from_parts, extern_params_equal};
use crate::diag::ErrorKind;
use sla_ast::decls::*;
use sla_ast::exprs::*;
use sla_ast::node::Node;
use sla_ast::stmts::*;
use sla_ast::types::*;

// -- builders (mirroring tests.rs) -------------------------------------

fn int_lit(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
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

fn ret_stmt(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt { value: Some(Box::new(value)) })
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

fn call(name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: name.to_string(),
        associated_target: None,
        generics: Vec::new(),
        args,
        callee: None,
    })
}

fn ud(name: &str, generics: Vec<Type>) -> Type {
    Type::UserDefined(UserDefined { name: name.to_string(), generics })
}

fn string_ty() -> Type {
    ud("String", Vec::new())
}

fn vec_u8_ty() -> Type {
    ud("Vec", vec![Type::Primitive(Primitive::U8)])
}

fn i32_ty() -> Type {
    Type::Primitive(Primitive::I32)
}

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
}

fn void_ty() -> Type {
    Type::Primitive(Primitive::VoidType)
}

fn borrow(e: Node) -> Node {
    Node::BorrowExpr(BorrowExpr { expr: Box::new(e) })
}


fn field(e: Node, name: &str) -> Node {
    Node::FieldExpr(FieldExpr {
        expr: Box::new(e),
        field_name: name.to_string(),
    })
}

fn struct_decl(name: &str, fields: Vec<(&str, Type)>) -> Node {
    Node::StructDecl(StructDecl {
        name: name.to_string(),
        derives: Vec::new(),
        generics: Vec::new(),
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

fn struct_lit(ty: Type, fields: Vec<(&str, Node)>) -> Node {
    Node::StructLiteral(StructLiteral {
        ty: Box::new(ty),
        fields: fields
            .into_iter()
            .map(|(n, v)| StructLiteralField {
                name: n.to_string(),
                value: Box::new(v),
            })
            .collect(),
        update_expr: None,
    })
}

fn check(decls: Vec<Node>) -> Result<(), crate::diag::Diagnostic> {
    let mut checker = Checker::new();
    let program = Program { decls };
    checker.check_program(&program)
}

// -- F1: lang-gap A_min fallible externs --------------------------------
// Corpus: neg_fallible_abi_payload / neg_fallible_bare_payload /
// neg_fallible_result_discarded.

fn fallible_extern(payload_spelling: &str) -> ExternSig {
    let parts = vec![("value".to_string(), "i32".to_string(), false, false)];
    let mut sig = extern_sig_from_parts(&parts, payload_spelling, true, false);
    sig.ret_abi = payload_spelling.to_string();
    sig
}

#[test]
fn r6_f1_fallible_payload_unsupported() {
    let mut checker = Checker::new();
    checker
        .register_extern_contract("sa_fixture_fallible_vec", fallible_extern("Vec"))
        .unwrap();
    let program = Program {
        decls: vec![func(
            "f",
            vec![],
            void_ty(),
            vec![let_stmt(
                "r",
                None,
                call("sa_fixture_fallible_vec", vec![int_lit(1)]),
            )],
        )],
    };
    let err = checker
        .check_program(&program)
        .expect_err("fallible Vec payload must be rejected");
    assert_eq!(err.kind, ErrorKind::FalliblePayloadUnsupported);
}

#[test]
fn r6_f1_fallible_result_discarded() {
    let mut checker = Checker::new();
    checker
        .register_extern_contract("sa_test_fallible_i32_value", fallible_extern("i32"))
        .unwrap();
    let program = Program {
        decls: vec![func(
            "f",
            vec![],
            i32_ty(),
            vec![
                expr_stmt(call("sa_test_fallible_i32_value", vec![int_lit(1)])),
                ret_stmt(int_lit(0)),
            ],
        )],
    };
    let err = checker
        .check_program(&program)
        .expect_err("bare fallible statement must be rejected");
    assert_eq!(err.kind, ErrorKind::FallibleResultDiscarded);
}

#[test]
fn r6_f1_fallible_bound_by_let_is_fine() {
    // Positive control: binding the fallible result consumes it.
    let mut checker = Checker::new();
    checker
        .register_extern_contract("sa_test_fallible_i32_value", fallible_extern("i32"))
        .unwrap();
    let program = Program {
        decls: vec![func(
            "f",
            vec![],
            i32_ty(),
            vec![
                let_stmt("r", None, call("sa_test_fallible_i32_value", vec![int_lit(1)])),
                ret_stmt(int_lit(0)),
            ],
        )],
    };
    let result = checker.check_program(&program);
    assert!(result.is_ok(), "{result:?}");
}

// -- F2: redeclaration ---------------------------------------------------
// Corpus: test_error_redeclaration_{function, const_function, macro,
// macro_param} + test_error_redeclaration_top_const (bonus).

#[test]
fn r6_f2_function_redeclaration() {
    let result = check(vec![
        func("repeated", vec![], i32_ty(), vec![ret_stmt(int_lit(1))]),
        func("repeated", vec![], i32_ty(), vec![ret_stmt(int_lit(2))]),
    ]);
    let err = result.expect_err("duplicate function must be rejected");
    assert_eq!(err.kind, ErrorKind::Redeclaration);
}

#[test]
fn r6_f2_macro_param_redeclaration() {
    let mac = Node::MacroDecl(MacroDecl {
        name: "dup".to_string(),
        params: vec!["value".to_string(), "value".to_string()],
        param_tys: vec![None],
        body: vec![let_stmt("result", None, ident("value"))],
    });
    let result = check(vec![mac]);
    let err = result.expect_err("duplicate macro parameter must be rejected");
    assert_eq!(err.kind, ErrorKind::Redeclaration);
}

// -- F3: view/borrow stage 1 ---------------------------------------------
// Corpus: test_error_view_local_same_field /
// test_error_view_root_reassign_while_borrowed /
// test_error_view_direct_write_borrowed_field.

fn pair_struct() -> Node {
    struct_decl("Pair", vec![("a", i64_ty()), ("b", i64_ty())])
}

fn pair_lit() -> Node {
    struct_lit(
        ud("Pair", Vec::new()),
        vec![("a", int_lit(1)), ("b", int_lit(2))],
    )
}

#[test]
fn r6_f3_local_same_field_borrow_conflict() {
    let result = check(vec![
        pair_struct(),
        func(
            "bad",
            vec![],
            void_ty(),
            vec![
                let_stmt("s", None, pair_lit()),
                let_stmt("x", None, borrow(field(ident("s"), "a"))),
                let_stmt("y", None, borrow(field(ident("s"), "a"))),
            ],
        ),
    ]);
    let err = result.expect_err("second borrow of the same field must fail");
    assert_eq!(err.kind, ErrorKind::InvalidBorrow);
    assert_eq!(err.marker, Some("BorrowConflict"));
}

#[test]
fn r6_f3_root_reassign_while_borrowed() {
    let result = check(vec![
        pair_struct(),
        func(
            "bad",
            vec![],
            void_ty(),
            vec![
                let_stmt("s", None, pair_lit()),
                let_stmt("x", None, borrow(field(ident("s"), "a"))),
                Node::AssignStmt(AssignStmt {
                    target: Box::new(ident("s")),
                    value: Box::new(pair_lit()),
                }),
            ],
        ),
    ]);
    let err = result.expect_err("root reassign under a live borrow must fail");
    assert_eq!(err.kind, ErrorKind::InvalidBorrow);
    assert_eq!(err.marker, Some("BorrowConflict"));
}

#[test]
fn r6_f3_disjoint_field_borrows_are_legal() {
    // Positive control: disjoint field borrows coexist (stage-1 rule).
    let result = check(vec![
        pair_struct(),
        func(
            "ok",
            vec![],
            void_ty(),
            vec![
                let_stmt("s", None, pair_lit()),
                let_stmt("x", None, borrow(field(ident("s"), "a"))),
                let_stmt("y", None, borrow(field(ident("s"), "b"))),
            ],
        ),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

// -- F4: overload resolution ---------------------------------------------
// Corpus: test_error_overload_literal_ambiguous /
// test_error_overload_generic_mix / test_error_overload_no_match.

#[test]
fn r6_f4_literal_ambiguous() {
    let result = check(vec![
        func("pick", vec![param("x", i32_ty())], i32_ty(), vec![ret_stmt(int_lit(1))]),
        func("pick", vec![param("x", i64_ty())], i32_ty(), vec![ret_stmt(int_lit(2))]),
        func(
            "main",
            vec![],
            i32_ty(),
            vec![ret_stmt(call("pick", vec![int_lit(7)]))],
        ),
    ]);
    let err = result.expect_err("untyped literal matching two members is ambiguous");
    assert_eq!(err.kind, ErrorKind::AmbiguousCall);
    assert!(err.message.contains("pick(i32)"), "{}", err.message);
    assert!(err.message.contains("pick(i64)"), "{}", err.message);
}

#[test]
fn r6_f4_typed_argument_narrows_to_one_member() {
    // Positive control for the typed-narrowing fix: a binding typed
    // i32 matches ONLY the i32 member (user ruling 2026-10-30:
    // exact match on inferred argument types). Corpus anchor:
    // test_unit_diag_trio_overloads_ok.
    let result = check(vec![
        func("pick", vec![param("x", i32_ty())], i32_ty(), vec![ret_stmt(int_lit(1))]),
        func(
            "pick",
            vec![param("x", Type::Primitive(Primitive::U64))],
            i32_ty(),
            vec![ret_stmt(int_lit(2))],
        ),
        func(
            "main",
            vec![],
            i32_ty(),
            vec![
                let_stmt("a", Some(i32_ty()), int_lit(3)),
                ret_stmt(call("pick", vec![ident("a")])),
            ],
        ),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn r6_f4_no_match_stays_rejected() {
    let result = check(vec![
        func("pick", vec![param("x", i32_ty())], i32_ty(), vec![ret_stmt(int_lit(1))]),
        func(
            "pick",
            vec![param("x", Type::Primitive(Primitive::F64))],
            i32_ty(),
            vec![ret_stmt(int_lit(2))],
        ),
        func(
            "main",
            vec![],
            i32_ty(),
            vec![ret_stmt(call(
                "pick",
                vec![Node::Literal(Literal::BoolVal(true))],
            ))],
        ),
    ]);
    let err = result.expect_err("bool argument matches no member");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

// -- F5: REG-BR-6 use-after-move by-value call args -----------------------
// Corpus: test_error_use_after_move_call /
// test_error_def_triage_nested_use_after_move. NOTE: this family is
// the subject of the open D-C7 doctrine fork escalated in RESULT.md;
// the test pins the STAGING (corpus-attested) shape - a SECOND CALL
// reusing the moved binding. It does not pin the bare-read shape,
// which is exactly what the fork adjudicates.

#[test]
fn r6_f5_second_call_rejected() {
    let borrow_only = func(
        "borrow_only",
        vec![param("s", string_ty())],
        void_ty(),
        vec![expr_stmt(bin(
            BinaryOp::Eq,
            call("len", vec![ident("s")]),
            int_lit(999999),
        ))],
    );
    let bad2 = func(
        "bad2",
        vec![param("x", string_ty())],
        void_ty(),
        vec![
            expr_stmt(call("borrow_only", vec![ident("x")])),
            expr_stmt(call("borrow_only", vec![ident("x")])),
        ],
    );
    let result = check(vec![borrow_only, bad2]);
    let err = result.expect_err("second by-value use must be rejected");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
    assert!(
        err.message.contains("borrow_only"),
        "{}",
        err.message
    );
}

// -- F6: FB-011 switch pattern checking -----------------------------------
// Corpus: test_error_fb011_switch_pattern_type /
// test_error_fb011_switch_undef_pattern.

#[test]
fn r6_f6_switch_pattern_type_mismatch() {
    let switch = Node::SwitchExpr(SwitchExpr {
        val: Box::new(ident("x")),
        cases: vec![
            Case {
                pattern: Box::new(Node::Literal(Literal::StringVal(
                    "a string against int".to_string(),
                ))),
                body: vec![ret_stmt(int_lit(1))],
            },
            Case {
                pattern: Box::new(ident("default")),
                body: vec![ret_stmt(int_lit(0))],
            },
        ],
    });
    let result = check(vec![func(
        "f",
        vec![param("x", i64_ty())],
        i64_ty(),
        vec![expr_stmt(switch)],
    )]);
    let err = result.expect_err("string pattern against i64 scrutinee must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn r6_f6_switch_undef_pattern() {
    let switch = Node::SwitchExpr(SwitchExpr {
        val: Box::new(ident("x")),
        cases: vec![
            Case {
                pattern: Box::new(bin(
                    BinaryOp::Add,
                    ident("y"),
                    ident("undefined_name"),
                )),
                body: vec![ret_stmt(int_lit(1))],
            },
            Case {
                pattern: Box::new(ident("default")),
                body: vec![ret_stmt(int_lit(0))],
            },
        ],
    });
    let result = check(vec![func(
        "f",
        vec![param("x", i64_ty())],
        i64_ty(),
        vec![expr_stmt(switch)],
    )]);
    let err = result.expect_err("undefined name in a pattern must fail");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
}

// -- F7: deep-copy gates ---------------------------------------------------
// Corpus: test_error_deep_copy_map_field_copy (UncloneableCopy) /
// test_error_deep_copy_literal_live_binding (UseAfterMove).

#[test]
fn r6_f7_uncloneable_preserved_argument() {
    let map_deep = struct_decl(
        "MapDeep",
        vec![
            ("m", ud("HashMap", vec![i32_ty(), i32_ty()])),
            ("n", i32_ty()),
        ],
    );
    let idm = func(
        "idm",
        vec![param("x", ud("MapDeep", Vec::new()))],
        ud("MapDeep", Vec::new()),
        vec![ret_stmt(ident("x"))],
    );
    let bad = func(
        "bad",
        vec![param("a", ud("MapDeep", Vec::new()))],
        i32_ty(),
        vec![
            let_stmt("b", None, call("idm", vec![ident("a")])),
            ret_stmt(bin(
                BinaryOp::Add,
                field(ident("a"), "n"),
                field(ident("b"), "n"),
            )),
        ],
    );
    let result = check(vec![map_deep, idm, bad]);
    let err = result.expect_err("preserved HashMap-closure argument must fail");
    assert_eq!(err.kind, ErrorKind::CompileError);
    assert_eq!(err.marker, Some("UncloneableCopy"));
}

#[test]
fn r6_f7_struct_literal_consumes_live_binding() {
    let deep = struct_decl("Deep", vec![("xs", vec_u8_ty()), ("n", i32_ty())]);
    let wrap = struct_decl(
        "Wrap",
        vec![("inner", ud("Deep", Vec::new())), ("tag", i32_ty())],
    );
    let bad = func(
        "bad",
        vec![param("d", ud("Deep", Vec::new()))],
        i32_ty(),
        vec![
            let_stmt(
                "w",
                None,
                struct_lit(
                    ud("Wrap", Vec::new()),
                    vec![("inner", ident("d")), ("tag", int_lit(7))],
                ),
            ),
            ret_stmt(bin(
                BinaryOp::Add,
                field(ident("d"), "n"),
                field(ident("w"), "tag"),
            )),
        ],
    );
    let result = check(vec![deep, wrap, bad]);
    let err = result.expect_err("use of a literal-consumed binding must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

// -- Loop guard + Fix A (return-value exemption) ---------------------------
// Corpus: test_error_loop_consume_fill (negative) /
// test_unit_loop_consume_return_fill (positive).

fn holder_struct() -> Node {
    struct_decl("LoopHolder", vec![("buf", vec_u8_ty()), ("tag", i32_ty())])
}

#[test]
fn r6_loop_consume_fill_rejected() {
    let bad = func(
        "bad_fill_loop",
        vec![param("seed", vec_u8_ty())],
        i32_ty(),
        vec![
            let_stmt("i", None, int_lit(0)),
            Node::WhileStmt(WhileStmt {
                cond: Box::new(bin(BinaryOp::Lt, ident("i"), int_lit(2))),
                let_pattern: None,
                body: vec![
                    let_stmt(
                        "h",
                        None,
                        struct_lit(
                            ud("LoopHolder", Vec::new()),
                            vec![("buf", ident("seed")), ("tag", ident("i"))],
                        ),
                    ),
                    Node::AssignStmt(AssignStmt {
                        target: Box::new(ident("i")),
                        value: Box::new(bin(BinaryOp::Add, ident("i"), int_lit(1))),
                    }),
                ],
            }),
            ret_stmt(int_lit(0)),
        ],
    );
    let result = check(vec![holder_struct(), bad]);
    let err = result.expect_err("loop-carried literal fill must fail");
    assert_eq!(err.kind, ErrorKind::CompileError);
    assert_eq!(err.marker, Some("LoopConditionalConsume"));
}

#[test]
fn r6_fix_a_return_fill_inside_loop_is_legal() {
    // Positive control (Fix A): consumption inside a RETURN value
    // leaves with the return, so the loop guard must not fire.
    // Corpus anchor: test_unit_loop_consume_return_fill (the
    // dns_read_name shape: conditional early return inside the
    // loop, final return after the loop).
    let out = struct_decl("LoopFillOut", vec![("buf", vec_u8_ty()), ("tag", i32_ty())]);
    let early_return = Node::IfExpr(IfExpr {
        cond: Box::new(bin(BinaryOp::Ge, ident("i"), int_lit(9))),
        let_chain: None,
        then_block: vec![ret_stmt(struct_lit(
            ud("LoopFillOut", Vec::new()),
            vec![("buf", ident("acc")), ("tag", int_lit(1))],
        ))],
        else_block: None,
    });
    let good = func(
        "fill",
        vec![param("acc", vec_u8_ty())],
        ud("LoopFillOut", Vec::new()),
        vec![
            let_stmt("i", None, int_lit(0)),
            Node::WhileStmt(WhileStmt {
                cond: Box::new(bin(BinaryOp::Lt, ident("i"), int_lit(2))),
                let_pattern: None,
                body: vec![
                    expr_stmt(early_return),
                    Node::AssignStmt(AssignStmt {
                        target: Box::new(ident("i")),
                        value: Box::new(bin(BinaryOp::Add, ident("i"), int_lit(1))),
                    }),
                ],
            }),
            ret_stmt(struct_lit(
                ud("LoopFillOut", Vec::new()),
                vec![("buf", ident("acc")), ("tag", int_lit(0))],
            )),
        ],
    );
    let result = check(vec![out, good]);
    assert!(result.is_ok(), "{result:?}");
}

// -- Fix B: scalar place reads are not borrows ------------------------------
// Corpus anchors: test_unit_bend2_parity_array /
// test_unit_vec_push_pod_struct_return_direct (positives).

#[test]
fn r6_fix_b_scalar_field_read_then_assign_is_legal() {
    let result = check(vec![
        pair_struct(),
        func(
            "ok",
            vec![],
            void_ty(),
            vec![
                let_stmt("s", None, pair_lit()),
                let_stmt("old", None, field(ident("s"), "a")),
                Node::AssignStmt(AssignStmt {
                    target: Box::new(field(ident("s"), "a")),
                    value: Box::new(int_lit(9)),
                }),
            ],
        ),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

