//! Regression tests for the stage-6 async cluster: the
//! future/task/executor/poll runtime-call plans, the await
//! readiness plan, the async function-return plan, and the
//! single-await continuation planner (mirrors of
//! lowering_rules.zig planFutureRuntimeCall /
//! planTaskRuntimeCall / planExecutorRuntimeCall /
//! planPollRuntimeCall / planAwaitFutureWithReadiness /
//! planAsyncFunctionReturn / planAsyncSingleAwaitContinuation).
//! Pre-fix these planners did not exist (the whole family was
//! registered deferred in deferred.rs); the emitter rejected
//! every async function with `async function (continuation
//! plans deferred)`.

use crate::async_plans::{plan_async_function_return, plan_async_single_await_continuation};
use crate::async_runtime::{
    expr_future_readiness, plan_await_future_with_readiness, plan_executor_runtime_call,
    plan_future_runtime_call, plan_poll_runtime_call, plan_task_runtime_call,
    ExecutorRuntimeCallKind, FutureReadiness, FutureRuntimeCallKind, PollRuntimeCallKind,
    TaskRuntimeCallKind,
};
use sla_ast::{
    AwaitExpr, CallExpr, FuncDecl, LetStmt, Literal, Node, Primitive, Type,
};

fn i64_ty() -> Type {
    Type::Primitive(Primitive::I64)
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

fn func_decl(is_async: bool, body: Vec<Node>) -> FuncDecl {
    FuncDecl {
        name: "f".to_string(),
        is_pub: false,
        is_extern: false,
        abi: None,
        no_mangle: false,
        is_decl_only: false,
        generics: Vec::new(),
        params: Vec::new(),
        ret_ty: Box::new(i64_ty()),
        body,
        is_inline: false,
        is_async,
        operator: None,
        is_arena: false,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
        is_variadic: false,
    }
}

#[test]
fn future_runtime_call_kinds() {
    let ready = call(Some("future"), "ready", vec![int(1)]);
    let Node::CallExpr(c) = &ready else { panic!() };
    let plan = plan_future_runtime_call(c).expect("future::ready plan");
    assert!(plan.is_ready());
    assert_eq!(plan.kind, FutureRuntimeCallKind::Ready);

    let defer = call(Some("future"), "defer_ready", vec![int(1)]);
    let Node::CallExpr(c) = &defer else { panic!() };
    assert!(plan_future_runtime_call(c).expect("defer_ready").is_defer_ready());

    let join = call(Some("future"), "join2", vec![ident("a"), ident("b")]);
    let Node::CallExpr(c) = &join else { panic!() };
    assert!(plan_future_runtime_call(c).expect("join2").is_join2());

    let left = call(Some("future"), "pair_left", vec![ident("p")]);
    let Node::CallExpr(c) = &left else { panic!() };
    let plan = plan_future_runtime_call(c).expect("pair_left");
    assert!(plan.is_pair_accessor());
    assert_eq!(plan.pair_macro_name(), Some("FUTURE_PAIR_LEFT"));

    // A non-future target is not a future runtime call.
    let other = call(Some("task"), "ready", vec![int(1)]);
    let Node::CallExpr(c) = &other else { panic!() };
    assert!(plan_future_runtime_call(c).is_none());
}

#[test]
fn task_executor_poll_runtime_call_kinds() {
    let new = call(Some("task"), "new", vec![ident("f")]);
    let Node::CallExpr(c) = &new else { panic!() };
    assert_eq!(
        plan_task_runtime_call(c).expect("task::new").kind,
        TaskRuntimeCallKind::New
    );

    let poll = call(Some("task"), "poll", vec![ident("t")]);
    let Node::CallExpr(c) = &poll else { panic!() };
    assert_eq!(
        plan_task_runtime_call(c).expect("task::poll").kind,
        TaskRuntimeCallKind::Poll
    );

    let count = call(Some("executor"), "poll_ready_count", vec![ident("e"), ident("b")]);
    let Node::CallExpr(c) = &count else { panic!() };
    assert_eq!(
        plan_executor_runtime_call(c).expect("poll_ready_count").kind,
        ExecutorRuntimeCallKind::PollReadyCount
    );

    let pr = call(Some("poll"), "ready", vec![int(3)]);
    let Node::CallExpr(c) = &pr else { panic!() };
    assert_eq!(
        plan_poll_runtime_call(c).expect("poll::ready").kind,
        PollRuntimeCallKind::Ready
    );
}

#[test]
fn readiness_and_await_plan() {
    let ready = call(Some("future"), "ready", vec![int(1)]);
    assert_eq!(
        expr_future_readiness(&ready, None),
        FutureReadiness::Ready
    );
    let pending = call(Some("future"), "pending", vec![]);
    assert_eq!(
        expr_future_readiness(&pending, None),
        FutureReadiness::Pending
    );

    // Awaiting a statically ready future whose inner type matches
    // the async return type takes the ready-state return path.
    let plan = plan_await_future_with_readiness(
        &ready,
        &future_i64_ty(),
        Some(&i64_ty()),
        None,
    );
    assert!(plan.ready_pending_state_return_if_async);
    assert!(!plan.pending_return_if_async);

    // Awaiting a statically pending future inside an async fn
    // takes the pending-return path.
    let plan = plan_await_future_with_readiness(
        &pending,
        &future_i64_ty(),
        Some(&i64_ty()),
        None,
    );
    assert!(plan.pending_return_if_async);
}

#[test]
fn function_return_plan_only_for_async() {
    let sync_plan = plan_async_function_return(&func_decl(false, vec![]));
    assert!(!sync_plan.abi_ret_is_ptr);
    assert!(!sync_plan.wrap_ready_future);
    let async_plan = plan_async_function_return(&func_decl(true, vec![]));
    assert!(async_plan.abi_ret_is_ptr);
    assert!(async_plan.wrap_ready_future);
}

#[test]
fn single_await_continuation_plan() {
    // async fn f() -> i64 { let v = future::defer_ready(41).await; v }
    let awaited = Node::AwaitExpr(AwaitExpr {
        expr: Box::new(call(Some("future"), "defer_ready", vec![int(41)])),
    });
    let body = vec![
        Node::LetStmt(LetStmt {
            name: "v".to_string(),
            ty: None,
            value: Box::new(awaited),
        }),
        Node::ExprStmt(Box::new(ident("v"))),
    ];
    let f = func_decl(true, body);
    let plan = plan_async_single_await_continuation(&f)
        .expect("single defer_ready await must plan");
    assert_eq!(plan.binding_name, "v");
    assert_eq!(plan.capture_count, 0);
    assert!(plan.scalar.is_identity());

    // The same body in a sync fn never plans.
    let f = func_decl(false, f.body.clone());
    assert!(plan_async_single_await_continuation(&f).is_none());

    // A non-defer_ready await (a plain ready future) is outside
    // the single-await family: the planner must decline loudly
    // (None) rather than mis-plan.
    let awaited_ready = Node::AwaitExpr(AwaitExpr {
        expr: Box::new(call(Some("future"), "ready", vec![int(41)])),
    });
    let body = vec![
        Node::LetStmt(LetStmt {
            name: "v".to_string(),
            ty: None,
            value: Box::new(awaited_ready),
        }),
        Node::ExprStmt(Box::new(ident("v"))),
    ];
    let f = func_decl(true, body);
    assert!(plan_async_single_await_continuation(&f).is_none());
}
