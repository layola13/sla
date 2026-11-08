//! Phase-2 expansion tests (checker line, second period): each test
//! names the Zig behavior it pins and, where the venue's real
//! plugin was consulted, the probe outcome. Venue probes ran
//! 2026-11-07 on venue 3 against the real plugin at head 6002b386
//! (`sa sla check`): switch-consume rc=1 (UseAfterMove),
//! switch-tail rc=0, if-let rc=0, let-else non-diverging rc=1
//! (TypeMismatch, "let else block must diverge"), let-else ok rc=0,
//! destructure-of-Copy-tuple rc=1 (UseAfterMove), match mismatch
//! rc=1 (TypeMismatch), match ok rc=0, while-let rc=0.

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

fn pick_decl() -> Node {
    Node::EnumDecl(EnumDecl {
        name: "Pick".to_string(),
        generics: Vec::new(),
        variants: vec![
            EnumVariant {
                name: "Have".to_string(),
                fields: vec![Field {
                    name: "x".to_string(),
                    ty: Box::new(i32_ty()),
                }],
                discriminant: None,
            },
            EnumVariant {
                name: "Lack".to_string(),
                fields: Vec::new(),
                discriminant: None,
            },
        ],
    })
}

fn pick_ty() -> Type {
    Type::UserDefined(UserDefined {
        name: "Pick".to_string(),
        generics: Vec::new(),
    })
}

fn match_pick(cases: Vec<MatchCase>) -> Node {
    Node::MatchExpr(MatchExpr {
        val: Box::new(ident("e")),
        cases,
    })
}

fn pick_case(variant: &str, bindings: Vec<&str>, body: Vec<Node>) -> MatchCase {
    MatchCase {
        pattern: EnumPattern {
            enum_name: "Pick".to_string(),
            variant_name: variant.to_string(),
            bindings: bindings.into_iter().map(|s| s.to_string()).collect(),
        },
        guard: None,
        body,
    }
}

fn check(decls: Vec<Node>) -> (Checker, Result<(), crate::diag::Diagnostic>) {
    let mut checker = Checker::new();
    let program = Program { decls };
    let result = checker.check_program(&program);
    (checker, result)
}

// -- item 1: switch --------------------------------------------------------

#[test]
fn switch_merge_consumes_single_case_binding() {
    // Zig switch arm + mergeMultiBranchScopeStates: `s` is consumed
    // in one live case and active in the other, so the intersect
    // merge consumes it (venue probe p_switch_consume: rc=1).
    let switch = Node::SwitchExpr(SwitchExpr {
        val: Box::new(ident("x")),
        cases: vec![
            Case {
                pattern: Box::new(int_lit(1)),
                body: vec![let_stmt("a", ident("s"))],
            },
            Case {
                pattern: Box::new(ident("default")),
                // A statement in the surviving branch: Zig records
                // the phi cleanup on the branch's last node, and
                // skips empty bodies entirely (body.len == 0).
                body: vec![expr_stmt(int_lit(0))],
            },
        ],
    });
    let (checker, result) = check(vec![func(
        "f",
        vec![param("s", string_ty()), param("x", i32_ty())],
        void_ty(),
        vec![expr_stmt(switch), let_stmt("b", ident("s"))],
    )]);
    let err = result.expect_err("post-switch use of s must fail");
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
fn switch_all_terminating_restores_pre_state() {
    // Both cases consume `s` but both terminate: no live branches,
    // the merge restores the pre-switch state and `s` stays usable.
    let switch = Node::SwitchExpr(SwitchExpr {
        val: Box::new(ident("x")),
        cases: vec![
            Case {
                pattern: Box::new(int_lit(1)),
                body: vec![let_stmt("a", ident("s")), ret(int_lit(1))],
            },
            Case {
                pattern: Box::new(ident("default")),
                body: vec![ret(int_lit(2))],
            },
        ],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("s", string_ty()), param("x", i32_ty())],
        i32_ty(),
        vec![
            expr_stmt(switch),
            let_stmt("b", ident("s")),
            expr_stmt(int_lit(0)),
        ],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn switch_tail_with_default_terminates_function() {
    // exprTerminates switch arm (Zig #7): all arms return and a
    // default arm exists, so the tail check is skipped
    // (venue probe p_switch_tail: rc=0).
    let switch = Node::SwitchExpr(SwitchExpr {
        val: Box::new(ident("x")),
        cases: vec![
            Case {
                pattern: Box::new(int_lit(1)),
                body: vec![ret(int_lit(1))],
            },
            Case {
                pattern: Box::new(ident("default")),
                body: vec![ret(int_lit(2))],
            },
        ],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("x", i32_ty())],
        i32_ty(),
        vec![expr_stmt(switch)],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn switch_tail_without_default_does_not_terminate() {
    // Without `default` a value can fall through: the switch is a
    // void-typed tail in an i32 function and the tail check bites.
    let switch = Node::SwitchExpr(SwitchExpr {
        val: Box::new(ident("x")),
        cases: vec![Case {
            pattern: Box::new(int_lit(1)),
            body: vec![ret(int_lit(1))],
        }],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("x", i32_ty())],
        i32_ty(),
        vec![expr_stmt(switch)],
    )]);
    let err = result.expect_err("void switch tail must fail the tail check");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

// -- item 1: match ---------------------------------------------------------

#[test]
fn match_enum_ok_with_value() {
    // Venue probe p_match_ok: rc=0.
    let m = match_pick(vec![
        pick_case("Have", vec!["x"], vec![expr_stmt(ident("x"))]),
        pick_case("Lack", vec![], vec![expr_stmt(int_lit(-1))]),
    ]);
    let (_, result) = check(vec![
        pick_decl(),
        func("f", vec![param("e", pick_ty())], i32_ty(), vec![expr_stmt(m)]),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn match_enum_case_type_mismatch_rejected() {
    // Venue probe p_match_mismatch: rc=1 TypeMismatch.
    let m = match_pick(vec![
        pick_case("Have", vec!["x"], vec![expr_stmt(ident("x"))]),
        pick_case(
            "Lack",
            vec![],
            vec![expr_stmt(Node::Literal(Literal::BoolVal(true)))],
        ),
    ]);
    let (_, result) = check(vec![
        pick_decl(),
        func("f", vec![param("e", pick_ty())], i32_ty(), vec![expr_stmt(m)]),
    ]);
    let err = result.expect_err("bool case in i32 match must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn match_unknown_variant_rejected() {
    let m = match_pick(vec![pick_case("Nope", vec![], vec![expr_stmt(int_lit(0))])]);
    let (_, result) = check(vec![
        pick_decl(),
        func("f", vec![param("e", pick_ty())], i32_ty(), vec![expr_stmt(m)]),
    ]);
    let err = result.expect_err("unknown variant must fail");
    assert_eq!(err.kind, ErrorKind::FieldNotFound);
}

#[test]
fn match_binding_arity_rejected() {
    let m = match_pick(vec![
        pick_case("Have", vec![], vec![expr_stmt(int_lit(0))]),
        pick_case("Lack", vec![], vec![expr_stmt(int_lit(1))]),
    ]);
    let (_, result) = check(vec![
        pick_decl(),
        func("f", vec![param("e", pick_ty())], i32_ty(), vec![expr_stmt(m)]),
    ]);
    let err = result.expect_err("missing Have binding must fail");
    assert_eq!(err.kind, ErrorKind::InvalidArgsCount);
}

#[test]
fn match_merge_consumes_single_case_binding() {
    // Same intersect rule as switch, through the enum path: `s`
    // consumed in the Have case only is consumed after the match.
    let m = Node::MatchExpr(MatchExpr {
        val: Box::new(ident("e")),
        cases: vec![
            pick_case("Have", vec!["x"], vec![let_stmt("a", ident("s"))]),
            pick_case("Lack", vec![], vec![]),
        ],
    });
    let (_, result) = check(vec![
        pick_decl(),
        func(
            "f",
            vec![param("e", pick_ty()), param("s", string_ty())],
            void_ty(),
            vec![expr_stmt(m), let_stmt("b", ident("s"))],
        ),
    ]);
    let err = result.expect_err("post-match use of s must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}

#[test]
fn match_option_some_binds_payload() {
    let m = Node::MatchExpr(MatchExpr {
        val: Box::new(ident("o")),
        cases: vec![
            MatchCase {
                pattern: EnumPattern {
                    enum_name: "Option".to_string(),
                    variant_name: "Some".to_string(),
                    bindings: vec!["v".to_string()],
                },
                guard: None,
                body: vec![expr_stmt(ident("v"))],
            },
            MatchCase {
                pattern: EnumPattern {
                    enum_name: "Option".to_string(),
                    variant_name: "None".to_string(),
                    bindings: vec![],
                },
                guard: None,
                body: vec![expr_stmt(int_lit(0))],
            },
        ],
    });
    let (_, result) = check(vec![func(
        "f",
        vec![param("o", option_ty(i32_ty()))],
        i32_ty(),
        vec![expr_stmt(m)],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn match_all_arms_terminate_terminates() {
    // exprTerminates match arm: every case body returns, so a tail
    // match satisfies a non-void function.
    let m = match_pick(vec![
        pick_case("Have", vec!["x"], vec![ret(ident("x"))]),
        pick_case("Lack", vec![], vec![ret(int_lit(0))]),
    ]);
    let (_, result) = check(vec![
        pick_decl(),
        func("f", vec![param("e", pick_ty())], i32_ty(), vec![expr_stmt(m)]),
    ]);
    assert!(result.is_ok(), "{result:?}");
}

// -- item 2: if-let --------------------------------------------------------

fn if_let(
    pattern: EnumPattern,
    value: Node,
    then_block: Vec<Node>,
    else_block: Option<Vec<Node>>,
) -> Node {
    Node::IfExpr(IfExpr {
        cond: Box::new(value.clone()),
        let_chain: Some(vec![IfLetCond {
            pattern,
            value: Box::new(value),
        }]),
        then_block,
        else_block,
    })
}

fn some_pattern(binding: &str) -> EnumPattern {
    EnumPattern {
        enum_name: "Option".to_string(),
        variant_name: "Some".to_string(),
        bindings: vec![binding.to_string()],
    }
}

#[test]
fn if_let_some_binds_and_returns() {
    // Venue probe p_iflet: rc=0.
    let expr = if_let(
        some_pattern("v"),
        ident("o"),
        vec![ret(ident("v"))],
        Some(vec![ret(int_lit(2))]),
    );
    let (_, result) = check(vec![func(
        "f",
        vec![param("o", option_ty(i32_ty()))],
        i32_ty(),
        vec![expr_stmt(expr)],
    )]);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn if_let_pattern_enum_mismatch_rejected() {
    // A Result pattern against an Option value: TypeMismatch from
    // patternBindingType ("if let pattern must match Option<T>").
    let expr = if_let(
        EnumPattern {
            enum_name: "Result".to_string(),
            variant_name: "Ok".to_string(),
            bindings: vec!["v".to_string()],
        },
        ident("o"),
        vec![expr_stmt(int_lit(1))],
        None,
    );
    let (_, result) = check(vec![func(
        "f",
        vec![param("o", option_ty(i32_ty()))],
        void_ty(),
        vec![expr_stmt(expr)],
    )]);
    let err = result.expect_err("Result pattern on Option must fail");
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
}

#[test]
fn if_let_else_consumption_merges() {
    // `s` consumed only in the else branch of an if-let: after the
    // merge it is consumed, like a plain if.
    let expr = if_let(
        some_pattern("v"),
        ident("o"),
        vec![expr_stmt(int_lit(1))],
        Some(vec![let_stmt("a", ident("s"))]),
    );
    let (_, result) = check(vec![func(
        "f",
        vec![param("o", option_ty(i32_ty())), param("s", string_ty())],
        void_ty(),
        vec![expr_stmt(expr), let_stmt("b", ident("s"))],
    )]);
    let err = result.expect_err("post-if-let use of s must fail");
    assert_eq!(err.kind, ErrorKind::UseAfterMove);
}
