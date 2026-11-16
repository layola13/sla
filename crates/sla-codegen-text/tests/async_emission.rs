//! Regression tests for the stage-6 async cluster (SA-text
//! emitter): the async function-return wrap, the future/task
//! runtime calls, the single-await continuation state machine,
//! and the FUTURE_NEW forget-form retire of the state operand.
//!
//! Pre-fix, `generate` rejected every async function with
//! `async function (continuation plans deferred)` and had no
//! runtime-call emission at all; the state-operand retire used
//! the release form (or nothing), which on the pinned SA host
//! either dropped the state block before the first poll or
//! leaked the register at function exit.

use sla_ast::{
    AwaitExpr, CallExpr, FuncDecl, LetStmt, Literal, Node, Primitive, Program, ReturnStmt, Type,
};
use sla_lowering::MapCheckInfo;

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
}

fn ptr_ty() -> Type {
    Type::Primitive(Primitive::RawPtr)
}

fn future_i64_ty() -> Type {
    Type::Future(Box::new(i64_ty()))
}

fn call(target: Option<&str>, func: &str, args: Vec<Node>) -> Node {
    Node::CallExpr(CallExpr {
        func_name: func.to_string(),
        associated_target: target.map(str::to_string),
        generics: Vec::new(),
        args,
        callee: None,
    })
}

fn int(v: i64) -> Node {
    Node::Literal(Literal::IntVal(v))
}

fn ident(name: &str) -> Node {
    Node::Identifier(name.to_string())
}

fn func(name: &str, is_async: bool, ret: Type, body: Vec<Node>) -> Node {
    Node::FuncDecl(FuncDecl {
        name: name.to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params: Vec::new(),
        ret_ty: Box::new(ret),
        body,
        is_inline: false,
        is_async,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
        is_variadic: false,
    })
}

fn program(decls: Vec<Node>) -> Node {
    Node::Program(Program { decls })
}

#[test]
fn async_fn_return_wraps_ready_future() {
    // async fn f() -> i64 { return 7; }
    let prog = program(vec![func(
        "f",
        true,
        i64_ty(),
        vec![Node::ReturnStmt(ReturnStmt {
            value: Some(Box::new(int(7))),
        })],
    )]);
    let mut check = MapCheckInfo::new();
    if let Node::Program(p) = &prog {
        if let Node::FuncDecl(f) = &p.decls[0] {
            if let Node::ReturnStmt(r) = &f.body[0] {
                check.add_expr_type(r.value.as_ref().unwrap(), i64_ty());
            }
        }
    }
    let out = sla_codegen_text::generate(&prog, &check)
        .expect("async fn must emit on the text backend (stage 6)");
    assert!(
        out.contains("@sla__f() -> ptr"),
        "async fn ABI returns a raw future-state pointer, got:\n{out}"
    );
    assert!(
        out.contains("FUTURE_READY_STATE_NEW"),
        "return value must be wrapped in a ready future, got:\n{out}"
    );
}

#[test]
fn future_ready_runtime_call_emits_helpers() {
    // fn g() -> ptr { let r = future::ready(7); return r; }
    let ready_call = call(Some("future"), "ready", vec![int(7)]);
    let prog = program(vec![func(
        "g",
        false,
        ptr_ty(),
        vec![
            Node::LetStmt(LetStmt {
                name: "r".to_string(),
                ty: None,
                value: Box::new(ready_call),
            }),
            Node::ReturnStmt(ReturnStmt {
                value: Some(Box::new(ident("r"))),
            }),
        ],
    )]);
    let mut check = MapCheckInfo::new();
    if let Node::Program(p) = &prog {
        if let Node::FuncDecl(f) = &p.decls[0] {
            if let Node::LetStmt(l) = &f.body[0] {
                check.add_expr_type(&l.value, future_i64_ty());
            }
            if let Node::ReturnStmt(r) = &f.body[1] {
                check.add_expr_type(r.value.as_ref().unwrap(), future_i64_ty());
            }
        }
    }
    let out = sla_codegen_text::generate(&prog, &check)
        .expect("future::ready must emit on the text backend (stage 6)");
    assert!(
        out.contains("SLA_READY_FUTURE_VT"),
        "the shared future/task helpers must be emitted, got:\n{out}"
    );
    assert!(
        out.contains("FUTURE_READY_STATE_NEW"),
        "future::ready must build a ready state, got:\n{out}"
    );
    assert!(
        out.contains("@import \"sa_std/core/task.sa\""),
        "the async macro family must import task.sa, got:\n{out}"
    );
}

#[test]
fn single_await_async_fn_emits_state_machine() {
    // async fn h() -> i64 { let v = future::defer_ready(41).await; v }
    let awaited = Node::AwaitExpr(AwaitExpr {
        expr: Box::new(call(Some("future"), "defer_ready", vec![int(41)])),
    });
    let prog = program(vec![func(
        "h",
        true,
        i64_ty(),
        vec![
            Node::LetStmt(LetStmt {
                name: "v".to_string(),
                ty: None,
                value: Box::new(awaited),
            }),
            Node::ExprStmt(Box::new(ident("v"))),
        ],
    )]);
    let check = MapCheckInfo::new();
    let out = sla_codegen_text::generate(&prog, &check)
        .expect("single-await async fn must emit on the text backend (stage 6)");
    assert!(
        out.contains("SLA_ASYNC_h_VT"),
        "single-await vtable const must be emitted, got:\n{out}"
    );
    assert!(
        out.contains("@sla_async_h_poll("),
        "single-await poll helper must be emitted, got:\n{out}"
    );
}

#[test]
fn task_new_future_new_state_operand_uses_forget_form() {
    // fn k() { let t = task::new(future::ready(1)); }
    let task_call = call(
        Some("task"),
        "new",
        vec![call(Some("future"), "ready", vec![int(1)])],
    );
    let prog = program(vec![func(
        "k",
        false,
        Type::Primitive(Primitive::VoidType),
        vec![
            Node::LetStmt(LetStmt {
                name: "t".to_string(),
                ty: None,
                value: Box::new(task_call),
            }),
            Node::ReturnStmt(ReturnStmt { value: None }),
        ],
    )]);
    let mut check = MapCheckInfo::new();
    if let Node::Program(p) = &prog {
        if let Node::FuncDecl(f) = &p.decls[0] {
            if let Node::LetStmt(l) = &f.body[0] {
                check.add_expr_type(&l.value, ptr_ty());
                if let Node::CallExpr(c) = &*l.value {
                    check.add_expr_type(&c.args[0], future_i64_ty());
                }
            }
        }
    }
    let out = sla_codegen_text::generate(&prog, &check)
        .expect("task::new(future::ready(..)) must emit (stage 6)");
    let new_pos = out
        .find("EXPAND FUTURE_NEW")
        .expect("the ready future must be wrapped in a future object");
    let after = &out[new_pos..];
    let new_line = after.lines().next().unwrap();
    // "EXPAND FUTURE_NEW <obj>, <state>, <vt>": the state operand
    // is the second operand.
    let state_operand = new_line
        .split(',')
        .nth(1)
        .map(|s| s.trim().to_string())
        .expect("FUTURE_NEW must name a state operand");
    let forget_form = format!("^{state_operand}");
    let retired_with_forget = after
        .lines()
        .take(4)
        .any(|l| l.trim() == forget_form);
    assert!(
        retired_with_forget,
        "the FUTURE_NEW state operand {state_operand} must be retired \
         with the forget form (release drops the state block on the \
         pinned host), got:\n{after}\nfull output:\n{out}"
    );
    let released_form = format!("!{state_operand}");
    assert!(
        !after.lines().take(4).any(|l| l.trim() == released_form),
        "the FUTURE_NEW state operand {state_operand} must not be \
         retired with the release form, got:\n{after}"
    );
    assert!(
        out.contains("EXPAND TASK_NEW"),
        "task::new must emit TASK_NEW, got:\n{out}"
    );
}
