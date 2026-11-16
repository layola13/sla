//! Stage 10 regression tests for RUST-S9-MINIM-WORKER-CAPABILITY.
//!
//! A `thread::spawn` worker body is generated from the spawn
//! helper's CLONED closure AST (`thread_spawn.rs` stores
//! `closure.clone()`), so node-address-keyed checker facts miss
//! for every node inside the body — including a call argument's
//! type, which decides Copy-ness in `gen_planned_call_arg`.
//! With no type the planner assumed an ownership transfer and
//! emitted `^reg` for the by-value parameter; the host rejects
//! that prefix for a Copy scalar, so the worker call failed at
//! assembly with CapabilityMismatch
//! (`call @sla__minim_worker(^tmp_26)` for `minim_worker(x: i64)`
//! called with a captured `cap: i64`).
//!
//! Captures are seeded as typed local bindings in the worker
//! prologue; the planner now falls back to the binding type for
//! identifier arguments (same fallback shape as the surface
//! receiver classification, RUST-S9-BARE-CLONE-CALL).
//!
//! These tests simulate the fact loss directly: NO checker
//! expr-type is registered anywhere — only the parameter
//! prologue's binding type. On the base emitter both tests fail
//! with the `^` move prefix on the call argument.

use sla_ast::*;
use sla_lowering::MapCheckInfo;

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
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

fn call(func_name: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: func_name.to_string(),
        associated_target: None,
        generics: Vec::new(),
        args,
        callee: None,
    })
}

fn ret(value: Node) -> Node {
    Node::ReturnStmt(ReturnStmt {
        value: Some(Box::new(value)),
    })
}

/// fn g(x: i64) -> i64 { return x; }
/// fn f(a: i64) -> i64 { return g(a); }
/// with NO checker facts anywhere — the worker-body situation.
fn factless_copy_arg_program() -> Node {
    Node::Program(Program {
        decls: vec![
            func(
                "g",
                vec![param("x", i64_ty())],
                i64_ty(),
                vec![ret(ident("x"))],
            ),
            func(
                "f",
                vec![param("a", i64_ty())],
                i64_ty(),
                vec![ret(call("g", vec![ident("a")]))],
            ),
        ],
    })
}

fn call_line<'o>(out: &'o str, symbol: &str) -> &'o str {
    out.lines()
        .find(|l| l.contains(&format!("call @{symbol}(")))
        .unwrap_or_else(|| panic!("expected a call to @{symbol}, got:\n{out}"))
}

#[test]
fn copy_scalar_arg_without_fact_is_passed_plain() {
    let program = factless_copy_arg_program();
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&program, &check)
        .expect("call with a Copy arg must emit without an arg fact");
    let line = call_line(&out, "sla__g");
    assert!(
        !line.contains("(^"),
        "a Copy scalar arg must not carry the ^ move prefix, got: {line}\nfull output:\n{out}"
    );
}

#[test]
fn copy_scalar_arg_without_fact_still_calls_callee() {
    let program = factless_copy_arg_program();
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&program, &check)
        .expect("call with a Copy arg must emit without an arg fact");
    assert!(
        out.contains("call @sla__g("),
        "expected a plain call to the callee, got:\n{out}"
    );
}
