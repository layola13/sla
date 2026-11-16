//! Runtime-call plans for the future/task/executor/poll surfaces and
//! the await/readiness facts, mirrored from lowering_rules.zig
//! (planFutureRuntimeCall 2938, planTaskRuntimeCall 2966,
//! planExecutorRuntimeCall 2977, planPollRuntimeCall 2986,
//! planAwaitFutureWithReadiness 2925, exprFutureReadiness 2838,
//! executorTaskBufferPlan 4662, asyncContinuationConditionOpName
//! 3946). Stage 6 async cluster: these plans were previously
//! registered as deferred in deferred.rs.

use std::collections::HashMap;

use sla_ast::{BinaryOp, CallExpr, Node, Type};

use crate::abi::peel_borrow_pointer_type;

// -- future runtime calls (lowering_rules.zig:306-375) ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FutureRuntimeCallKind {
    Ready,
    Pending,
    DeferReady,
    Join2,
    PairLeft,
    PairRight,
    Select2,
    EitherSide,
    EitherLeft,
    EitherRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FutureRuntimeCallPlan {
    pub kind: FutureRuntimeCallKind,
}

impl FutureRuntimeCallPlan {
    pub fn is_ready(self) -> bool {
        self.kind == FutureRuntimeCallKind::Ready
    }
    pub fn is_pending(self) -> bool {
        self.kind == FutureRuntimeCallKind::Pending
    }
    pub fn is_defer_ready(self) -> bool {
        self.kind == FutureRuntimeCallKind::DeferReady
    }
    pub fn is_join2(self) -> bool {
        self.kind == FutureRuntimeCallKind::Join2
    }
    pub fn is_select2(self) -> bool {
        self.kind == FutureRuntimeCallKind::Select2
    }
    pub fn is_pair_accessor(self) -> bool {
        self.pair_macro_name().is_some()
    }
    pub fn is_either_accessor(self) -> bool {
        self.either_value_macro_name().is_some()
    }
    pub fn pair_macro_name(self) -> Option<&'static str> {
        match self.kind {
            FutureRuntimeCallKind::PairLeft => Some("FUTURE_PAIR_LEFT"),
            FutureRuntimeCallKind::PairRight => Some("FUTURE_PAIR_RIGHT"),
            _ => None,
        }
    }
    pub fn either_value_macro_name(self) -> Option<&'static str> {
        match self.kind {
            FutureRuntimeCallKind::EitherSide => Some("FUTURE_EITHER_SIDE"),
            FutureRuntimeCallKind::EitherLeft => Some("FUTURE_EITHER_LEFT_VALUE"),
            FutureRuntimeCallKind::EitherRight => Some("FUTURE_EITHER_RIGHT_VALUE"),
            _ => None,
        }
    }
}

/// lowering_rules.zig:2938 `planFutureRuntimeCall`.
pub fn plan_future_runtime_call(call: &CallExpr) -> Option<FutureRuntimeCallPlan> {
    let kind = |k| Some(FutureRuntimeCallPlan { kind: k });
    if let Some(target) = &call.associated_target {
        if target != "future" {
            return None;
        }
        return match call.func_name.as_str() {
            "ready" => kind(FutureRuntimeCallKind::Ready),
            "pending" => kind(FutureRuntimeCallKind::Pending),
            "defer_ready" => kind(FutureRuntimeCallKind::DeferReady),
            "join2" => kind(FutureRuntimeCallKind::Join2),
            "pair_left" => kind(FutureRuntimeCallKind::PairLeft),
            "pair_right" => kind(FutureRuntimeCallKind::PairRight),
            "select2" => kind(FutureRuntimeCallKind::Select2),
            "either_side" => kind(FutureRuntimeCallKind::EitherSide),
            "either_left" => kind(FutureRuntimeCallKind::EitherLeft),
            "either_right" => kind(FutureRuntimeCallKind::EitherRight),
            _ => None,
        };
    }
    match call.func_name.as_str() {
        "future__ready" => kind(FutureRuntimeCallKind::Ready),
        "future__pending" => kind(FutureRuntimeCallKind::Pending),
        "future__defer_ready" => kind(FutureRuntimeCallKind::DeferReady),
        "future__join2" => kind(FutureRuntimeCallKind::Join2),
        "future__pair_left" => kind(FutureRuntimeCallKind::PairLeft),
        "future__pair_right" => kind(FutureRuntimeCallKind::PairRight),
        "future__select2" => kind(FutureRuntimeCallKind::Select2),
        "future__either_side" => kind(FutureRuntimeCallKind::EitherSide),
        "future__either_left" => kind(FutureRuntimeCallKind::EitherLeft),
        "future__either_right" => kind(FutureRuntimeCallKind::EitherRight),
        _ => None,
    }
}

// -- task runtime calls ------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskRuntimeCallKind {
    New,
    Poll,
    IsReady,
    Result,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskRuntimeCallPlan {
    pub kind: TaskRuntimeCallKind,
}

impl TaskRuntimeCallPlan {
    pub fn is_new(self) -> bool {
        self.kind == TaskRuntimeCallKind::New
    }
    pub fn is_poll(self) -> bool {
        self.kind == TaskRuntimeCallKind::Poll
    }
    pub fn is_is_ready(self) -> bool {
        self.kind == TaskRuntimeCallKind::IsReady
    }
    pub fn is_result(self) -> bool {
        self.kind == TaskRuntimeCallKind::Result
    }
    pub fn is_state(self) -> bool {
        self.kind == TaskRuntimeCallKind::State
    }
}

/// lowering_rules.zig:2966 `planTaskRuntimeCall`.
pub fn plan_task_runtime_call(call: &CallExpr) -> Option<TaskRuntimeCallPlan> {
    let target = call.associated_target.as_deref()?;
    if target != "task" {
        return None;
    }
    let kind = match call.func_name.as_str() {
        "new" => TaskRuntimeCallKind::New,
        "poll" => TaskRuntimeCallKind::Poll,
        "is_ready" => TaskRuntimeCallKind::IsReady,
        "result" => TaskRuntimeCallKind::Result,
        "state" => TaskRuntimeCallKind::State,
        _ => return None,
    };
    Some(TaskRuntimeCallPlan { kind })
}

// -- executor runtime calls ---------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorRuntimeCallKind {
    New,
    PollOne,
    PollReadyCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutorRuntimeCallPlan {
    pub kind: ExecutorRuntimeCallKind,
}

impl ExecutorRuntimeCallPlan {
    pub fn is_new(self) -> bool {
        self.kind == ExecutorRuntimeCallKind::New
    }
    pub fn is_poll_one(self) -> bool {
        self.kind == ExecutorRuntimeCallKind::PollOne
    }
    pub fn is_poll_ready_count(self) -> bool {
        self.kind == ExecutorRuntimeCallKind::PollReadyCount
    }
}

/// lowering_rules.zig:2977 `planExecutorRuntimeCall`.
pub fn plan_executor_runtime_call(call: &CallExpr) -> Option<ExecutorRuntimeCallPlan> {
    let target = call.associated_target.as_deref()?;
    if target != "executor" {
        return None;
    }
    let kind = match call.func_name.as_str() {
        "new" => ExecutorRuntimeCallKind::New,
        "poll_one" => ExecutorRuntimeCallKind::PollOne,
        "poll_ready_count" => ExecutorRuntimeCallKind::PollReadyCount,
        _ => return None,
    };
    Some(ExecutorRuntimeCallPlan { kind })
}

// -- poll runtime calls ---------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollRuntimeCallKind {
    Ready,
    Pending,
    IsReady,
    IsPending,
    Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollRuntimeCallPlan {
    pub kind: PollRuntimeCallKind,
}

impl PollRuntimeCallPlan {
    pub fn is_ready(self) -> bool {
        self.kind == PollRuntimeCallKind::Ready
    }
    pub fn is_pending(self) -> bool {
        self.kind == PollRuntimeCallKind::Pending
    }
    pub fn is_status_check(self) -> bool {
        matches!(
            self.kind,
            PollRuntimeCallKind::IsReady | PollRuntimeCallKind::IsPending
        )
    }
    pub fn is_value(self) -> bool {
        self.kind == PollRuntimeCallKind::Value
    }
    pub fn poll_status_macro_name(self) -> Option<&'static str> {
        match self.kind {
            PollRuntimeCallKind::IsReady => Some("POLL_IS_READY"),
            PollRuntimeCallKind::IsPending => Some("POLL_IS_PENDING"),
            _ => None,
        }
    }
}

/// lowering_rules.zig:2986 `planPollRuntimeCall`.
pub fn plan_poll_runtime_call(call: &CallExpr) -> Option<PollRuntimeCallPlan> {
    let kind = |k| Some(PollRuntimeCallPlan { kind: k });
    if let Some(target) = &call.associated_target {
        if target != "poll" {
            return None;
        }
        return match call.func_name.as_str() {
            "ready" => kind(PollRuntimeCallKind::Ready),
            "pending" => kind(PollRuntimeCallKind::Pending),
            "is_ready" => kind(PollRuntimeCallKind::IsReady),
            "is_pending" => kind(PollRuntimeCallKind::IsPending),
            "value" => kind(PollRuntimeCallKind::Value),
            _ => None,
        };
    }
    match call.func_name.as_str() {
        "poll__ready" => kind(PollRuntimeCallKind::Ready),
        "poll__pending" => kind(PollRuntimeCallKind::Pending),
        "poll__is_ready" => kind(PollRuntimeCallKind::IsReady),
        "poll__is_pending" => kind(PollRuntimeCallKind::IsPending),
        "poll__value" => kind(PollRuntimeCallKind::Value),
        _ => None,
    }
}

// -- readiness / await plan (lowering_rules.zig:2838-2936) ---------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FutureReadiness {
    Unknown,
    Ready,
    Pending,
}

/// lowering_rules.zig:2794 `join2Readiness`.
pub fn join2_readiness(left: FutureReadiness, right: FutureReadiness) -> FutureReadiness {
    if left == FutureReadiness::Ready && right == FutureReadiness::Ready {
        return FutureReadiness::Ready;
    }
    if left == FutureReadiness::Pending || right == FutureReadiness::Pending {
        return FutureReadiness::Pending;
    }
    FutureReadiness::Unknown
}

/// lowering_rules.zig:2801 `select2Readiness`.
pub fn select2_readiness(left: FutureReadiness, right: FutureReadiness) -> FutureReadiness {
    if left == FutureReadiness::Ready || right == FutureReadiness::Ready {
        return FutureReadiness::Ready;
    }
    if left == FutureReadiness::Pending && right == FutureReadiness::Pending {
        return FutureReadiness::Pending;
    }
    FutureReadiness::Unknown
}

/// lowering_rules.zig:2810 `exprFutureReadiness`.
pub fn expr_future_readiness(
    expr: &Node,
    readiness_by_name: Option<&HashMap<String, FutureReadiness>>,
) -> FutureReadiness {
    if let Node::Identifier(name) = expr {
        if let Some(map) = readiness_by_name {
            return map.get(name).copied().unwrap_or(FutureReadiness::Unknown);
        }
        return FutureReadiness::Unknown;
    }
    let call = match expr {
        Node::CallExpr(c) => c,
        _ => return FutureReadiness::Unknown,
    };
    let plan = match plan_future_runtime_call(call) {
        Some(p) => p,
        None => return FutureReadiness::Unknown,
    };
    match plan.kind {
        FutureRuntimeCallKind::Ready => FutureReadiness::Ready,
        FutureRuntimeCallKind::Pending => FutureReadiness::Pending,
        FutureRuntimeCallKind::Join2 => {
            if call.args.len() == 2 {
                join2_readiness(
                    expr_future_readiness(&call.args[0], readiness_by_name),
                    expr_future_readiness(&call.args[1], readiness_by_name),
                )
            } else {
                FutureReadiness::Unknown
            }
        }
        FutureRuntimeCallKind::Select2 => {
            if call.args.len() == 2 {
                select2_readiness(
                    expr_future_readiness(&call.args[0], readiness_by_name),
                    expr_future_readiness(&call.args[1], readiness_by_name),
                )
            } else {
                FutureReadiness::Unknown
            }
        }
        _ => FutureReadiness::Unknown,
    }
}

/// lowering_rules.zig:2880 `exprNeedsPollOnceForReadyAwait`.
pub fn expr_needs_poll_once_for_ready_await(
    expr: &Node,
    readiness_by_name: Option<&HashMap<String, FutureReadiness>>,
) -> bool {
    let call = match expr {
        Node::CallExpr(c) => c,
        _ => return false,
    };
    let plan = match plan_future_runtime_call(call) {
        Some(p) => p,
        None => return false,
    };
    match plan.kind {
        FutureRuntimeCallKind::Join2 | FutureRuntimeCallKind::Select2 => {
            expr_future_readiness(expr, readiness_by_name) == FutureReadiness::Ready
        }
        _ => false,
    }
}

/// lowering_rules.zig:2866 `futureInnerType`.
pub fn future_inner_type(ty: &Type) -> Option<&Type> {
    let curr = peel_borrow_pointer_type(ty);
    match curr {
        Type::Future(inner) => Some(inner),
        _ => None,
    }
}

/// lowering_rules.zig:2870 `typesEquivalent`.
pub fn types_equivalent(a: &Type, b: &Type) -> bool {
    use sla_ast::Type as T;
    if matches!(a, T::Infer) || matches!(b, T::Infer) {
        return true;
    }
    match (a, b) {
        (T::Primitive(pa), T::Primitive(pb)) => pa == pb,
        (T::Pointer(pa), T::Pointer(pb)) => types_equivalent(pa, pb),
        (T::Borrow(ba), T::Borrow(bb)) => types_equivalent(ba, bb),
        (T::Array(aa), T::Array(ab)) => {
            aa.len == ab.len && types_equivalent(&aa.elem, &ab.elem)
        }
        (T::Tuple(ta), T::Tuple(tb)) => {
            ta.elems.len() == tb.elems.len()
                && ta
                    .elems
                    .iter()
                    .zip(tb.elems.iter())
                    .all(|(ea, eb)| types_equivalent(ea, eb))
        }
        (T::Future(fa), T::Future(fb)) => types_equivalent(fa, fb),
        (T::Closure(ca), T::Closure(cb)) => {
            ca.params.len() == cb.params.len()
                && ca
                    .params
                    .iter()
                    .zip(cb.params.iter())
                    .all(|(pa, pb)| types_equivalent(pa, pb))
                && types_equivalent(&ca.ret, &cb.ret)
        }
        (T::FnPtr(fa), T::FnPtr(fb)) => {
            fa.abi == fb.abi
                && fa.params.len() == fb.params.len()
                && fa
                    .params
                    .iter()
                    .zip(fb.params.iter())
                    .all(|(pa, pb)| types_equivalent(pa, pb))
                && types_equivalent(&fa.ret, &fb.ret)
        }
        (T::UserDefined(ua), T::UserDefined(ub)) => {
            ua.name == ub.name
                && ua.generics.len() == ub.generics.len()
                && ua
                    .generics
                    .iter()
                    .zip(ub.generics.iter())
                    .all(|(ga, gb)| types_equivalent(ga, gb))
        }
        _ => false,
    }
}

/// lowering_rules.zig `AwaitPlan` + `planAwaitFutureWithReadiness`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AwaitPlan {
    pub ready_state_inner: bool,
    pub pending_return_if_async: bool,
    pub ready_pending_state_return_if_async: bool,
    pub poll_once_if_statically_ready: bool,
}

pub fn plan_await_future_with_readiness(
    expr: &Node,
    future_ty: &Type,
    async_return_ty: Option<&Type>,
    readiness_by_name: Option<&HashMap<String, FutureReadiness>>,
) -> AwaitPlan {
    let inner = future_inner_type(future_ty);
    AwaitPlan {
        ready_state_inner: true,
        pending_return_if_async: expr_future_readiness(expr, readiness_by_name)
            == FutureReadiness::Pending,
        poll_once_if_statically_ready: expr_needs_poll_once_for_ready_await(
            expr,
            readiness_by_name,
        ),
        ready_pending_state_return_if_async: match async_return_ty {
            Some(ret_ty) => match inner {
                Some(inner_ty) => types_equivalent(inner_ty, ret_ty),
                None => false,
            },
            None => false,
        },
    }
}

/// lowering_rules.zig:2905 `awaitExprFutureIsCustom`: the awaited
/// expression produces a future whose state does NOT follow the
/// ready-state {stage, value} protocol (a call to an async fn, or a
/// join2/select2/defer_ready construction).
pub fn await_expr_future_is_custom(
    expr: &Node,
    is_async_func: &dyn Fn(&str) -> bool,
) -> bool {
    let call = match expr {
        Node::CallExpr(c) => c,
        _ => return false,
    };
    if let Some(rt) = plan_future_runtime_call(call) {
        return matches!(
            rt.kind,
            FutureRuntimeCallKind::Join2
                | FutureRuntimeCallKind::Select2
                | FutureRuntimeCallKind::DeferReady
        );
    }
    is_async_func(&call.func_name)
}

// -- executor task buffer (lowering_rules.zig:4662) ----------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorTaskBufferKind {
    FixedArray,
    Vec,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecutorTaskBufferPlan {
    pub kind: ExecutorTaskBufferKind,
    pub inner: Type,
    pub fixed_len: Option<usize>,
}

impl ExecutorTaskBufferPlan {
    pub fn is_fixed_array(self: &ExecutorTaskBufferPlan) -> bool {
        self.kind == ExecutorTaskBufferKind::FixedArray
    }
    pub fn is_vec(self: &ExecutorTaskBufferPlan) -> bool {
        self.kind == ExecutorTaskBufferKind::Vec
    }
}

/// lowering_rules.zig `taskInnerType`: user-defined `Task<T>` inner.
fn task_inner_type(ty: &Type) -> Option<&Type> {
    match peel_borrow_pointer_type(ty) {
        Type::UserDefined(ud) if ud.name == "Task" && ud.generics.len() == 1 => {
            Some(&ud.generics[0])
        }
        _ => None,
    }
}

/// lowering_rules.zig `vecElementType` (shape used by the executor
/// plan): user-defined `Vec<T>` element.
fn vec_element_type(ty: &Type) -> Option<&Type> {
    match peel_borrow_pointer_type(ty) {
        Type::UserDefined(ud) if ud.name == "Vec" && ud.generics.len() == 1 => {
            Some(&ud.generics[0])
        }
        _ => None,
    }
}

/// lowering_rules.zig:4662 `executorTaskBufferPlan`.
pub fn executor_task_buffer_plan(ty: &Type) -> Option<ExecutorTaskBufferPlan> {
    let curr = peel_borrow_pointer_type(ty);
    if let Type::Array(arr) = curr {
        let inner = task_inner_type(&arr.elem)?;
        return Some(ExecutorTaskBufferPlan {
            kind: ExecutorTaskBufferKind::FixedArray,
            inner: inner.clone(),
            fixed_len: Some(arr.len),
        });
    }
    let elem = vec_element_type(curr)?;
    let inner = task_inner_type(elem)?;
    Some(ExecutorTaskBufferPlan {
        kind: ExecutorTaskBufferKind::Vec,
        inner: inner.clone(),
        fixed_len: None,
    })
}

/// lowering_rules.zig:3946 `asyncContinuationConditionOpName`.
pub fn async_continuation_condition_op_name(op: BinaryOp) -> Option<&'static str> {
    match op {
        BinaryOp::Eq => Some("eq"),
        BinaryOp::Ne => Some("ne"),
        BinaryOp::Lt => Some("slt"),
        BinaryOp::Le => Some("sle"),
        BinaryOp::Gt => Some("sgt"),
        BinaryOp::Ge => Some("sge"),
        _ => None,
    }
}
