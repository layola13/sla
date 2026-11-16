//! Async continuation planners, mirrored from lowering_rules.zig:
//! planAsyncFunctionReturn (1427), the AsyncContinuationAddend scalar
//! machinery (1436-1840), planAsyncSingleAwaitContinuation (2695),
//! planAsyncTwoAwaitContinuation (2132), planAsyncLinearAwait-
//! Continuation (2237) and planAsyncJoin2AwaitContinuation (2092).
//! General/loop planners live in async_plans_loop.rs. Stage 6 async
//! cluster; previously registered as deferred in deferred.rs.

use sla_ast::{BinaryOp, FuncDecl, Literal, Node};

use crate::async_runtime::{plan_future_runtime_call, FutureRuntimeCallKind};

// -- function return plan (lowering_rules.zig:1427) ----------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncReturnPlan {
    /// Async functions return a raw pointer (the future state).
    pub abi_ret_is_ptr: bool,
    pub wrap_ready_future: bool,
}

pub fn plan_async_function_return(func: &FuncDecl) -> AsyncReturnPlan {
    AsyncReturnPlan {
        abi_ret_is_ptr: func.is_async,
        wrap_ready_future: func.is_async,
    }
}

// -- scalar plan (lowering_rules.zig:187-199) -----------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncContinuationScalarPlan {
    pub awaited_coeff: i64,
    pub captured_coeff: i64,
    pub captured2_coeff: i64,
    pub captured_field_name: Option<String>,
    pub captured2_field_name: Option<String>,
    pub immediate: i64,
}

impl Default for AsyncContinuationScalarPlan {
    fn default() -> Self {
        AsyncContinuationScalarPlan {
            awaited_coeff: 1,
            captured_coeff: 0,
            captured2_coeff: 0,
            captured_field_name: None,
            captured2_field_name: None,
            immediate: 0,
        }
    }
}

impl AsyncContinuationScalarPlan {
    pub fn is_identity(&self) -> bool {
        self.awaited_coeff == 1
            && self.captured_coeff == 0
            && self.captured2_coeff == 0
            && self.immediate == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncContinuationCaptureStorage {
    Scalar,
    CopyStruct,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncContinuationCapturePlan<'a> {
    pub name: String,
    pub expr: &'a Node,
    pub offset: usize,
    pub size: usize,
    pub storage: AsyncContinuationCaptureStorage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncContinuationBranchPlan {
    pub condition_op: BinaryOp,
    pub threshold: i64,
    pub then_scalar: AsyncContinuationScalarPlan,
    pub else_scalar: AsyncContinuationScalarPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncSingleAwaitContinuationPlan<'a> {
    pub binding_name: String,
    pub post_binding_name: Option<String>,
    pub captured_addend_name: Option<String>,
    pub captured_addend_expr: Option<&'a Node>,
    pub capture_count: usize,
    pub captures: [Option<AsyncContinuationCapturePlan<'a>>; 2],
    pub await_expr: &'a Node,
    pub awaited_kind: FutureRuntimeCallKind,
    pub addend: i64,
    pub scalar: AsyncContinuationScalarPlan,
    pub branch: Option<AsyncContinuationBranchPlan>,
}

impl<'a> AsyncSingleAwaitContinuationPlan<'a> {
    pub fn result_binding_name(&self) -> Option<&str> {
        self.post_binding_name.as_deref()
    }
    pub fn has_captured_addend(&self) -> bool {
        self.capture_count != 0 || self.captured_addend_expr.is_some()
    }
    pub fn async_state_size(&self) -> usize {
        16 + self.capture_count * 8
    }
}

// -- addend machinery (lowering_rules.zig:1436-1690) ----------------------

#[derive(Debug, Clone, Default, PartialEq)]
struct AsyncContinuationAddend {
    immediate: i64,
    has_immediate: bool,
    has_await_binding: bool,
    captured_addend_name: Option<String>,
    captured2_addend_name: Option<String>,
    captured_field_name: Option<String>,
    captured2_field_name: Option<String>,
    awaited_coeff: i64,
    captured_coeff: i64,
    captured2_coeff: i64,
}

fn int_literal(expr: &Node) -> Option<i64> {
    match expr {
        Node::Literal(Literal::IntVal(v)) => Some(*v),
        _ => None,
    }
}

fn capture_name_index(name: &str, capture_names: &[String]) -> Option<usize> {
    capture_names.iter().position(|c| c == name)
}

fn merge_optional_capture_field(existing: &mut Option<String>, field_name: Option<&str>) -> bool {
    if let Some(current) = &*existing {
        let next = match field_name {
            Some(f) => f,
            None => return false,
        };
        return current == next;
    }
    if let Some(field) = field_name {
        *existing = Some(field.to_string());
    }
    true
}

fn add_captured_term(
    addend: &mut AsyncContinuationAddend,
    name: &str,
    field_name: Option<&str>,
    coeff: i64,
    capture_names: &[String],
) -> bool {
    let idx = match capture_name_index(name, capture_names) {
        Some(i) => i,
        None => return false,
    };
    match idx {
        0 => {
            if let Some(existing) = &addend.captured_addend_name {
                if existing != name {
                    return false;
                }
            } else {
                addend.captured_addend_name = Some(name.to_string());
            }
            if !merge_optional_capture_field(&mut addend.captured_field_name, field_name) {
                return false;
            }
            addend.captured_coeff = match addend.captured_coeff.checked_add(coeff) {
                Some(v) => v,
                None => return false,
            };
            true
        }
        1 => {
            if let Some(existing) = &addend.captured2_addend_name {
                if existing != name {
                    return false;
                }
            } else {
                addend.captured2_addend_name = Some(name.to_string());
            }
            if !merge_optional_capture_field(&mut addend.captured2_field_name, field_name) {
                return false;
            }
            addend.captured2_coeff = match addend.captured2_coeff.checked_add(coeff) {
                Some(v) => v,
                None => return false,
            };
            true
        }
        _ => false,
    }
}

fn scale_async_continuation_addend(addend: &mut AsyncContinuationAddend, factor: i64) -> bool {
    for v in [
        &mut addend.immediate,
        &mut addend.awaited_coeff,
        &mut addend.captured_coeff,
        &mut addend.captured2_coeff,
    ] {
        *v = match v.checked_mul(factor) {
            Some(x) => x,
            None => return false,
        };
    }
    true
}

fn merge_addend_field_name(target: &mut Option<String>, field_name: Option<&str>, coeff: i64) -> bool {
    if coeff == 0 && field_name.is_none() {
        return true;
    }
    merge_optional_capture_field(target, field_name)
}

fn add_async_continuation_addend(
    target: &mut AsyncContinuationAddend,
    addend: &AsyncContinuationAddend,
) -> bool {
    if addend.has_await_binding {
        target.has_await_binding = true;
    }
    target.immediate = match target.immediate.checked_add(addend.immediate) {
        Some(v) => v,
        None => return false,
    };
    target.awaited_coeff = match target.awaited_coeff.checked_add(addend.awaited_coeff) {
        Some(v) => v,
        None => return false,
    };
    if addend.has_immediate {
        target.has_immediate = true;
    }
    if let Some(captured) = &addend.captured_addend_name {
        if let Some(existing) = &target.captured_addend_name {
            if existing != captured {
                return false;
            }
        } else {
            target.captured_addend_name = Some(captured.clone());
        }
        if !merge_addend_field_name(
            &mut target.captured_field_name,
            addend.captured_field_name.as_deref(),
            addend.captured_coeff,
        ) {
            return false;
        }
        target.captured_coeff = match target.captured_coeff.checked_add(addend.captured_coeff) {
            Some(v) => v,
            None => return false,
        };
    }
    if let Some(captured) = &addend.captured2_addend_name {
        if let Some(existing) = &target.captured2_addend_name {
            if existing != captured {
                return false;
            }
        } else {
            target.captured2_addend_name = Some(captured.clone());
        }
        if !merge_addend_field_name(
            &mut target.captured2_field_name,
            addend.captured2_field_name.as_deref(),
            addend.captured2_coeff,
        ) {
            return false;
        }
        target.captured2_coeff = match target.captured2_coeff.checked_add(addend.captured2_coeff) {
            Some(v) => v,
            None => return false,
        };
    }
    true
}

fn collect_async_continuation_addend(
    expr: &Node,
    binding_name: &str,
    capture_names: &[String],
    addend: &mut AsyncContinuationAddend,
) -> bool {
    match expr {
        Node::Identifier(name) => {
            if name == binding_name {
                addend.has_await_binding = true;
                addend.awaited_coeff += 1;
                return true;
            }
            add_captured_term(addend, name, None, 1, capture_names)
        }
        Node::FieldExpr(field) => {
            let base = match &*field.expr {
                Node::Identifier(n) => n.clone(),
                _ => return false,
            };
            add_captured_term(addend, &base, Some(&field.field_name), 1, capture_names)
        }
        Node::Literal(lit) => {
            let v = match lit {
                Literal::IntVal(v) => *v,
                _ => return false,
            };
            if addend.has_immediate {
                return false;
            }
            addend.immediate = v;
            addend.has_immediate = true;
            true
        }
        Node::BinaryExpr(bin) => {
            if bin.op == BinaryOp::Mul {
                let left_lit = int_literal(&bin.left);
                let right_lit = int_literal(&bin.right);
                if left_lit.is_some() && right_lit.is_some() {
                    return false;
                }
                let factor = match left_lit.or(right_lit) {
                    Some(f) => f,
                    None => return false,
                };
                let expr_side = if left_lit.is_some() { &bin.right } else { &bin.left };
                let mut nested = AsyncContinuationAddend::default();
                if !collect_async_continuation_addend(expr_side, binding_name, capture_names, &mut nested) {
                    return false;
                }
                if !scale_async_continuation_addend(&mut nested, factor) {
                    return false;
                }
                return add_async_continuation_addend(addend, &nested);
            }
            if bin.op != BinaryOp::Add && bin.op != BinaryOp::Sub {
                return false;
            }
            let mut left = AsyncContinuationAddend::default();
            let mut right = AsyncContinuationAddend::default();
            if !collect_async_continuation_addend(&bin.left, binding_name, capture_names, &mut left) {
                return false;
            }
            if !collect_async_continuation_addend(&bin.right, binding_name, capture_names, &mut right) {
                return false;
            }
            if bin.op == BinaryOp::Sub && !scale_async_continuation_addend(&mut right, -1) {
                return false;
            }
            add_async_continuation_addend(addend, &left) && add_async_continuation_addend(addend, &right)
        }
        _ => false,
    }
}

fn addend_has_invalid_capture(addend: &AsyncContinuationAddend) -> bool {
    (addend.captured_addend_name.is_none() && addend.captured_coeff != 0)
        || (addend.captured2_addend_name.is_none() && addend.captured2_coeff != 0)
}

fn async_continuation_addend(
    expr: &Node,
    binding_name: &str,
    capture_names: &[String],
) -> Option<AsyncContinuationAddend> {
    let mut addend = AsyncContinuationAddend::default();
    if !collect_async_continuation_addend(expr, binding_name, capture_names, &mut addend) {
        return None;
    }
    if !addend.has_await_binding || addend.awaited_coeff == 0 {
        return None;
    }
    if addend_has_invalid_capture(&addend) {
        return None;
    }
    Some(addend)
}

fn async_continuation_scalar_expr(
    expr: &Node,
    binding_name: &str,
    capture_names: &[String],
) -> Option<AsyncContinuationAddend> {
    let mut addend = AsyncContinuationAddend::default();
    if !collect_async_continuation_addend(expr, binding_name, capture_names, &mut addend) {
        return None;
    }
    if addend_has_invalid_capture(&addend) {
        return None;
    }
    Some(addend)
}

pub fn async_continuation_return_expr<'a>(stmt: &'a Node) -> Option<&'a Node> {
    match stmt {
        Node::ReturnStmt(ret) => ret.value.as_deref(),
        Node::ExprStmt(expr) => Some(expr),
        _ => None,
    }
}

fn scalar_plan_from_addend(addend: &AsyncContinuationAddend) -> AsyncContinuationScalarPlan {
    AsyncContinuationScalarPlan {
        awaited_coeff: addend.awaited_coeff,
        captured_coeff: addend.captured_coeff,
        captured2_coeff: addend.captured2_coeff,
        captured_field_name: addend.captured_field_name.clone(),
        captured2_field_name: addend.captured2_field_name.clone(),
        immediate: addend.immediate,
    }
}

#[allow(dead_code)]
fn compose_post_return_scalar(
    binding: &AsyncContinuationAddend,
    ret: &AsyncContinuationAddend,
) -> Option<AsyncContinuationScalarPlan> {
    if ret.captured_coeff != 0
        || ret.captured_addend_name.is_some()
        || ret.captured2_coeff != 0
        || ret.captured2_addend_name.is_some()
    {
        return None;
    }
    let mut scalar = scalar_plan_from_addend(binding);
    scalar.awaited_coeff = scalar.awaited_coeff.checked_mul(ret.awaited_coeff)?;
    scalar.captured_coeff = scalar.captured_coeff.checked_mul(ret.awaited_coeff)?;
    scalar.captured2_coeff = scalar.captured2_coeff.checked_mul(ret.awaited_coeff)?;
    scalar.immediate = scalar.immediate.checked_mul(ret.awaited_coeff)?;
    scalar.immediate = scalar.immediate.checked_add(ret.immediate)?;
    Some(scalar)
}

struct ScalarFieldMerge {
    field_name: Option<String>,
}

fn merge_scalar_field_name(
    base_field: Option<&str>,
    base_coeff: i64,
    expr_field: Option<&str>,
    expr_coeff: i64,
) -> Option<ScalarFieldMerge> {
    let base_active = base_coeff != 0 || base_field.is_some();
    let expr_active = expr_coeff != 0 || expr_field.is_some();
    if !base_active {
        return Some(ScalarFieldMerge {
            field_name: expr_field.map(|s| s.to_string()),
        });
    }
    if !expr_active {
        return Some(ScalarFieldMerge {
            field_name: base_field.map(|s| s.to_string()),
        });
    }
    if let Some(base) = base_field {
        let expr = expr_field?;
        if base != expr {
            return None;
        }
        return Some(ScalarFieldMerge {
            field_name: Some(base.to_string()),
        });
    }
    if expr_field.is_some() {
        return None;
    }
    Some(ScalarFieldMerge { field_name: None })
}

fn compose_continuation_scalar(
    base: &AsyncContinuationScalarPlan,
    expr: &AsyncContinuationAddend,
) -> Option<AsyncContinuationScalarPlan> {
    let awaited_coeff = base.awaited_coeff.checked_mul(expr.awaited_coeff)?;
    let base_captured = base.captured_coeff.checked_mul(expr.awaited_coeff)?;
    let captured_coeff = base_captured.checked_add(expr.captured_coeff)?;
    let base_captured2 = base.captured2_coeff.checked_mul(expr.awaited_coeff)?;
    let captured2_coeff = base_captured2.checked_add(expr.captured2_coeff)?;
    let captured_field_name = merge_scalar_field_name(
        base.captured_field_name.as_deref(),
        base_captured,
        expr.captured_field_name.as_deref(),
        expr.captured_coeff,
    )?
    .field_name;
    let captured2_field_name = merge_scalar_field_name(
        base.captured2_field_name.as_deref(),
        base_captured2,
        expr.captured2_field_name.as_deref(),
        expr.captured2_coeff,
    )?
    .field_name;
    let base_immediate = base.immediate.checked_mul(expr.awaited_coeff)?;
    let immediate = base_immediate.checked_add(expr.immediate)?;
    Some(AsyncContinuationScalarPlan {
        awaited_coeff,
        captured_coeff,
        captured2_coeff,
        captured_field_name,
        captured2_field_name,
        immediate,
    })
}

fn capture_name_for_scalar(captured_name: Option<&str>, coeff: i64) -> Option<String> {
    if coeff != 0 {
        captured_name.map(|s| s.to_string())
    } else {
        None
    }
}

fn continuation_block_tail_expr(block: &[Node]) -> Option<&Node> {
    if block.len() != 1 {
        return None;
    }
    match &block[0] {
        Node::ExprStmt(expr) => Some(expr),
        _ => None,
    }
}

fn branch_condition_plan(cond: &Node, binding_name: &str) -> Option<(BinaryOp, i64)> {
    let bin = match cond {
        Node::BinaryExpr(b) => b,
        _ => return None,
    };
    if !matches!(
        bin.op,
        BinaryOp::Gt | BinaryOp::Ge | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Eq | BinaryOp::Ne
    ) {
        return None;
    }
    match &*bin.left {
        Node::Identifier(name) if name == binding_name => {}
        _ => return None,
    }
    let threshold = int_literal(&bin.right)?;
    Some((bin.op, threshold))
}

fn merge_branch_capture_name(left: Option<&str>, right: Option<&str>) -> Option<Option<String>> {
    // Returns None on conflict; Some(None) when both sides are absent.
    if let Some(l) = left {
        if let Some(r) = right {
            if l != r {
                return None;
            }
        }
        return Some(Some(l.to_string()));
    }
    Some(right.map(|s| s.to_string()))
}

fn merge_branch_capture_field(
    left: Option<&str>,
    right: Option<&str>,
    left_active: bool,
    right_active: bool,
) -> Option<ScalarFieldMerge> {
    if !left_active {
        return Some(ScalarFieldMerge {
            field_name: right.map(|s| s.to_string()),
        });
    }
    if !right_active {
        return Some(ScalarFieldMerge {
            field_name: left.map(|s| s.to_string()),
        });
    }
    if let Some(l) = left {
        let r = right?;
        if l != r {
            return None;
        }
        return Some(ScalarFieldMerge {
            field_name: Some(l.to_string()),
        });
    }
    if right.is_some() {
        return None;
    }
    Some(ScalarFieldMerge { field_name: None })
}

struct AsyncContinuationResult {
    post_binding_name: Option<String>,
    addend: i64,
    captured_addend_name: Option<String>,
    captured2_addend_name: Option<String>,
    captured_field_name: Option<String>,
    captured2_field_name: Option<String>,
    scalar: AsyncContinuationScalarPlan,
    branch: Option<AsyncContinuationBranchPlan>,
}

impl Default for AsyncContinuationResult {
    fn default() -> Self {
        AsyncContinuationResult {
            post_binding_name: None,
            addend: 0,
            captured_addend_name: None,
            captured2_addend_name: None,
            captured_field_name: None,
            captured2_field_name: None,
            scalar: AsyncContinuationScalarPlan::default(),
            branch: None,
        }
    }
}

fn async_continuation_branch(
    expr: &Node,
    binding_name: &str,
    capture_name: Option<&str>,
) -> Option<AsyncContinuationResult> {
    let ife = match expr {
        Node::IfExpr(i) => i,
        _ => return None,
    };
    if ife.let_chain.is_some() {
        return None;
    }
    let else_block = ife.else_block.as_ref()?;
    let (op, threshold) = branch_condition_plan(&ife.cond, binding_name)?;
    let then_expr = continuation_block_tail_expr(&ife.then_block)?;
    let else_expr = continuation_block_tail_expr(else_block)?;
    let capture_names: Vec<String> = match capture_name {
        Some(c) => vec![c.to_string()],
        None => Vec::new(),
    };
    let then_addend = async_continuation_scalar_expr(then_expr, binding_name, &capture_names)?;
    let else_addend = async_continuation_scalar_expr(else_expr, binding_name, &capture_names)?;
    // mergeBranchCaptureResult
    let first = merge_branch_capture_name(
        then_addend.captured_addend_name.as_deref(),
        else_addend.captured_addend_name.as_deref(),
    )?;
    if (then_addend.captured_addend_name.is_some() || else_addend.captured_addend_name.is_some())
        && first.is_none()
    {
        return None;
    }
    let second = merge_branch_capture_name(
        then_addend.captured2_addend_name.as_deref(),
        else_addend.captured2_addend_name.as_deref(),
    )?;
    if (then_addend.captured2_addend_name.is_some() || else_addend.captured2_addend_name.is_some())
        && second.is_none()
    {
        return None;
    }
    let first_field = merge_branch_capture_field(
        then_addend.captured_field_name.as_deref(),
        else_addend.captured_field_name.as_deref(),
        then_addend.captured_addend_name.is_some() || then_addend.captured_coeff != 0,
        else_addend.captured_addend_name.is_some() || else_addend.captured_coeff != 0,
    )?
    .field_name;
    let second_field = merge_branch_capture_field(
        then_addend.captured2_field_name.as_deref(),
        else_addend.captured2_field_name.as_deref(),
        then_addend.captured2_addend_name.is_some() || then_addend.captured2_coeff != 0,
        else_addend.captured2_addend_name.is_some() || else_addend.captured2_coeff != 0,
    )?
    .field_name;
    Some(AsyncContinuationResult {
        captured_addend_name: first,
        captured2_addend_name: second,
        captured_field_name: first_field,
        captured2_field_name: second_field,
        branch: Some(AsyncContinuationBranchPlan {
            condition_op: op,
            threshold,
            then_scalar: scalar_plan_from_addend(&then_addend),
            else_scalar: scalar_plan_from_addend(&else_addend),
        }),
        ..Default::default()
    })
}

fn async_continuation_result(
    await_binding_name: &str,
    capture_names: &[String],
    post_stmts: &[Node],
    ret_expr: &Node,
) -> Option<AsyncContinuationResult> {
    if !post_stmts.is_empty() {
        if post_stmts.len() > 2 {
            return None;
        }
        let mut current_name = await_binding_name.to_string();
        // Zig: `var current_scalar = AsyncContinuationScalarPlan{}`
        // — the struct defaults (awaited_coeff 1, rest zero).
        let mut current_scalar = AsyncContinuationScalarPlan::default();
        let mut current_capture_name: Option<String> = None;
        let mut current_capture2_name: Option<String> = None;
        let mut current_capture_field_name: Option<String> = None;
        let mut current_capture2_field_name: Option<String> = None;
        let mut post_binding_name: Option<String> = None;
        for stmt in post_stmts {
            let let_stmt = match stmt {
                Node::LetStmt(l) => l,
                _ => return None,
            };
            let addend = async_continuation_scalar_expr(&let_stmt.value, &current_name, capture_names)?;
            if !addend.has_await_binding || addend.awaited_coeff == 0 {
                return None;
            }
            current_scalar = compose_continuation_scalar(&current_scalar, &addend)?;
            if let Some(c) = &addend.captured_addend_name {
                current_capture_name = Some(c.clone());
            }
            if let Some(c) = &addend.captured2_addend_name {
                current_capture2_name = Some(c.clone());
            }
            if let Some(f) = &addend.captured_field_name {
                current_capture_field_name = Some(f.clone());
            }
            if let Some(f) = &addend.captured2_field_name {
                current_capture2_field_name = Some(f.clone());
            }
            current_name = let_stmt.name.clone();
            post_binding_name = Some(let_stmt.name.clone());
        }
        if let Node::Identifier(name) = ret_expr {
            if name == &current_name {
                let captured = capture_name_for_scalar(
                    current_capture_name.as_deref(),
                    current_scalar.captured_coeff,
                );
                let captured2 = capture_name_for_scalar(
                    current_capture2_name.as_deref(),
                    current_scalar.captured2_coeff,
                );
                let captured_field = capture_name_for_scalar(
                    current_capture_field_name.as_deref(),
                    current_scalar.captured_coeff,
                );
                let captured2_field = capture_name_for_scalar(
                    current_capture2_field_name.as_deref(),
                    current_scalar.captured2_coeff,
                );
                return Some(AsyncContinuationResult {
                    post_binding_name,
                    addend: current_scalar.immediate,
                    captured_addend_name: captured,
                    captured2_addend_name: captured2,
                    captured_field_name: captured_field,
                    captured2_field_name: captured2_field,
                    scalar: current_scalar,
                    branch: None,
                });
            }
        }
        let return_addend = async_continuation_scalar_expr(ret_expr, &current_name, &[])?;
        if !return_addend.has_await_binding || return_addend.awaited_coeff == 0 {
            return None;
        }
        let scalar = compose_continuation_scalar(&current_scalar, &return_addend)?;
        let captured = capture_name_for_scalar(current_capture_name.as_deref(), scalar.captured_coeff);
        let captured2 =
            capture_name_for_scalar(current_capture2_name.as_deref(), scalar.captured2_coeff);
        let captured_field =
            capture_name_for_scalar(current_capture_field_name.as_deref(), scalar.captured_coeff);
        let captured2_field = capture_name_for_scalar(
            current_capture2_field_name.as_deref(),
            scalar.captured2_coeff,
        );
        return Some(AsyncContinuationResult {
            post_binding_name,
            addend: scalar.immediate,
            captured_addend_name: captured,
            captured2_addend_name: captured2,
            captured_field_name: captured_field,
            captured2_field_name: captured2_field,
            scalar,
            branch: None,
        });
    }

    if capture_names.len() <= 1 {
        let capture_name = capture_names.first().map(|s| s.as_str());
        if let Some(branch) = async_continuation_branch(ret_expr, await_binding_name, capture_name) {
            return Some(branch);
        }
    }

    let addend = async_continuation_addend(ret_expr, await_binding_name, capture_names)?;
    Some(AsyncContinuationResult {
        addend: addend.immediate,
        captured_addend_name: addend.captured_addend_name.clone(),
        captured2_addend_name: addend.captured2_addend_name.clone(),
        captured_field_name: addend.captured_field_name.clone(),
        captured2_field_name: addend.captured2_field_name.clone(),
        scalar: scalar_plan_from_addend(&addend),
        ..Default::default()
    })
}

// -- await binding shapes (lowering_rules.zig:1803-1840) -------------------

pub fn stmt_is_prebound_await_state(stmt: &Node, next_stmt: &Node) -> bool {
    let state_let = match stmt {
        Node::LetStmt(l) => l,
        _ => return false,
    };
    let await_let = match next_stmt {
        Node::LetStmt(l) => l,
        _ => return false,
    };
    if !matches!(&*state_let.value, Node::CallExpr(_)) {
        return false;
    }
    let awaited_expr = match &*await_let.value {
        Node::AwaitExpr(a) => &a.expr,
        _ => return false,
    };
    match &**awaited_expr {
        Node::Identifier(name) => name == &state_let.name,
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncAwaitBindingShape<'a> {
    pub binding_name: String,
    pub state_expr: &'a Node,
    pub next_idx: usize,
}

pub fn async_await_binding_at<'a>(func: &'a FuncDecl, idx: usize) -> Option<AsyncAwaitBindingShape<'a>> {
    if idx >= func.body.len() {
        return None;
    }
    let await_let = match &func.body[idx] {
        Node::LetStmt(l) => l,
        _ => return None,
    };
    if let Node::AwaitExpr(a) = &*await_let.value {
        return Some(AsyncAwaitBindingShape {
            binding_name: await_let.name.clone(),
            state_expr: &a.expr,
            next_idx: idx + 1,
        });
    }
    if idx + 1 >= func.body.len() || !stmt_is_prebound_await_state(&func.body[idx], &func.body[idx + 1]) {
        return None;
    }
    let state_let = match &func.body[idx] {
        Node::LetStmt(l) => l,
        _ => unreachable!(),
    };
    let bound_await_let = match &func.body[idx + 1] {
        Node::LetStmt(l) => l,
        _ => unreachable!(),
    };
    Some(AsyncAwaitBindingShape {
        binding_name: bound_await_let.name.clone(),
        state_expr: &state_let.value,
        next_idx: idx + 2,
    })
}

// -- single await (lowering_rules.zig:2695) --------------------------------

pub fn plan_async_single_await_continuation<'a>(
    func: &'a FuncDecl,
) -> Option<AsyncSingleAwaitContinuationPlan<'a>> {
    if !func.is_async {
        return None;
    }
    let mut idx = 0usize;
    let mut capture_count = 0usize;
    let mut captures: [Option<AsyncContinuationCapturePlan<'a>>; 2] = [None, None];
    let mut capture_names: Vec<String> = Vec::new();
    while capture_count < 2 && idx < func.body.len() {
        let capture_let = match &func.body[idx] {
            Node::LetStmt(l) => l,
            _ => break,
        };
        let is_direct_await = matches!(&*capture_let.value, Node::AwaitExpr(_));
        let is_prebound_state = idx + 1 < func.body.len()
            && stmt_is_prebound_await_state(&func.body[idx], &func.body[idx + 1]);
        if is_direct_await || is_prebound_state {
            break;
        }
        captures[capture_count] = Some(AsyncContinuationCapturePlan {
            name: capture_let.name.clone(),
            expr: &capture_let.value,
            offset: 16 + capture_count * 8,
            size: 8,
            storage: AsyncContinuationCaptureStorage::Scalar,
        });
        capture_names.push(capture_let.name.clone());
        capture_count += 1;
        idx += 1;
    }

    if idx >= func.body.len() {
        return None;
    }
    if !matches!(&func.body[idx], Node::LetStmt(_)) {
        return None;
    }
    let first_await_let = match &func.body[idx] {
        Node::LetStmt(l) => l,
        _ => unreachable!(),
    };
    let (binding_name, state_expr): (String, &'a Node) = if let Node::AwaitExpr(a) = &*first_await_let.value {
        idx += 1;
        (first_await_let.name.clone(), &a.expr)
    } else {
        if idx + 1 >= func.body.len()
            || !stmt_is_prebound_await_state(&func.body[idx], &func.body[idx + 1])
        {
            return None;
        }
        let state_let = match &func.body[idx] {
            Node::LetStmt(l) => l,
            _ => unreachable!(),
        };
        let await_let = match &func.body[idx + 1] {
            Node::LetStmt(l) => l,
            _ => unreachable!(),
        };
        idx += 2;
        (await_let.name.clone(), &state_let.value)
    };

    if idx >= func.body.len() {
        return None;
    }
    let post_stmts = &func.body[idx..func.body.len() - 1];
    let ret_expr = async_continuation_return_expr(&func.body[func.body.len() - 1])?;

    let awaited_call = match state_expr {
        Node::CallExpr(c) => plan_future_runtime_call(c)?,
        _ => return None,
    };
    if awaited_call.kind != FutureRuntimeCallKind::DeferReady {
        return None;
    }

    let result = async_continuation_result(&binding_name, &capture_names, post_stmts, ret_expr)?;
    let used_capture_count = (result.captured_addend_name.is_some() as usize)
        + (result.captured2_addend_name.is_some() as usize);
    if capture_count != used_capture_count {
        return None;
    }

    let mut plan_captures: [Option<AsyncContinuationCapturePlan<'a>>; 2] = [None, None];
    if let Some(captured) = &result.captured_addend_name {
        let mut first = captures[0].take()?;
        if first.name != *captured {
            return None;
        }
        first.storage = if result.captured_field_name.is_some() {
            AsyncContinuationCaptureStorage::CopyStruct
        } else {
            AsyncContinuationCaptureStorage::Scalar
        };
        plan_captures[0] = Some(first);
    }
    if let Some(captured) = &result.captured2_addend_name {
        let mut second = captures[1].take()?;
        if second.name != *captured {
            return None;
        }
        second.storage = if result.captured2_field_name.is_some() {
            AsyncContinuationCaptureStorage::CopyStruct
        } else {
            AsyncContinuationCaptureStorage::Scalar
        };
        plan_captures[1] = Some(second);
    }
    let captured_addend_expr = plan_captures[0].as_ref().map(|c| c.expr);
    Some(AsyncSingleAwaitContinuationPlan {
        binding_name,
        post_binding_name: result.post_binding_name,
        captured_addend_name: result.captured_addend_name,
        captured_addend_expr,
        capture_count: used_capture_count,
        captures: plan_captures,
        await_expr: state_expr,
        awaited_kind: awaited_call.kind,
        addend: result.addend,
        scalar: result.scalar,
        branch: result.branch,
    })
}

// -- two await (lowering_rules.zig:1841-1925, 2132) -------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AsyncTwoAwaitScalarPlan {
    pub first_coeff: i64,
    pub second_coeff: i64,
    pub immediate: i64,
}

impl Default for AsyncTwoAwaitScalarPlan {
    fn default() -> Self {
        AsyncTwoAwaitScalarPlan {
            first_coeff: 1,
            second_coeff: 1,
            immediate: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncTwoAwaitContinuationPlan<'a> {
    pub first_binding_name: String,
    pub second_binding_name: String,
    pub first_await_expr: &'a Node,
    pub second_await_expr: &'a Node,
    pub first_awaited_kind: FutureRuntimeCallKind,
    pub second_awaited_kind: FutureRuntimeCallKind,
    pub scalar: AsyncTwoAwaitScalarPlan,
}

impl<'a> AsyncTwoAwaitContinuationPlan<'a> {
    pub fn async_state_size(&self) -> usize {
        32
    }
}

#[derive(Debug, Clone, Default)]
struct AsyncTwoAwaitAddend {
    has_first: bool,
    has_second: bool,
    first_coeff: i64,
    second_coeff: i64,
    immediate: i64,
}

fn scale_two_addend(addend: &mut AsyncTwoAwaitAddend, factor: i64) -> bool {
    for v in [&mut addend.first_coeff, &mut addend.second_coeff, &mut addend.immediate] {
        *v = match v.checked_mul(factor) {
            Some(x) => x,
            None => return false,
        };
    }
    true
}

fn add_two_addend(target: &mut AsyncTwoAwaitAddend, addend: &AsyncTwoAwaitAddend) -> bool {
    if addend.has_first {
        target.has_first = true;
    }
    if addend.has_second {
        target.has_second = true;
    }
    for (dst, src) in [
        (&mut target.first_coeff, addend.first_coeff),
        (&mut target.second_coeff, addend.second_coeff),
        (&mut target.immediate, addend.immediate),
    ] {
        *dst = match dst.checked_add(src) {
            Some(v) => v,
            None => return false,
        };
    }
    true
}

fn collect_two_addend(
    expr: &Node,
    first_name: &str,
    second_name: &str,
    addend: &mut AsyncTwoAwaitAddend,
) -> bool {
    match expr {
        Node::Identifier(name) => {
            if name == first_name {
                addend.has_first = true;
                addend.first_coeff = match addend.first_coeff.checked_add(1) {
                    Some(v) => v,
                    None => return false,
                };
                return true;
            }
            if name == second_name {
                addend.has_second = true;
                addend.second_coeff = match addend.second_coeff.checked_add(1) {
                    Some(v) => v,
                    None => return false,
                };
                return true;
            }
            false
        }
        Node::Literal(Literal::IntVal(v)) => {
            addend.immediate = match addend.immediate.checked_add(*v) {
                Some(x) => x,
                None => return false,
            };
            true
        }
        Node::BinaryExpr(bin) => {
            if bin.op == BinaryOp::Mul {
                let left_lit = int_literal(&bin.left);
                let right_lit = int_literal(&bin.right);
                if left_lit.is_some() && right_lit.is_some() {
                    return false;
                }
                let factor = match left_lit.or(right_lit) {
                    Some(f) => f,
                    None => return false,
                };
                let expr_side = if left_lit.is_some() { &bin.right } else { &bin.left };
                let mut nested = AsyncTwoAwaitAddend::default();
                if !collect_two_addend(expr_side, first_name, second_name, &mut nested) {
                    return false;
                }
                if !scale_two_addend(&mut nested, factor) {
                    return false;
                }
                return add_two_addend(addend, &nested);
            }
            if bin.op != BinaryOp::Add && bin.op != BinaryOp::Sub {
                return false;
            }
            let mut left = AsyncTwoAwaitAddend::default();
            let mut right = AsyncTwoAwaitAddend::default();
            if !collect_two_addend(&bin.left, first_name, second_name, &mut left) {
                return false;
            }
            if !collect_two_addend(&bin.right, first_name, second_name, &mut right) {
                return false;
            }
            if bin.op == BinaryOp::Sub && !scale_two_addend(&mut right, -1) {
                return false;
            }
            add_two_addend(addend, &left) && add_two_addend(addend, &right)
        }
        _ => false,
    }
}

fn async_two_await_scalar_expr(
    expr: &Node,
    first_name: &str,
    second_name: &str,
) -> Option<AsyncTwoAwaitScalarPlan> {
    let mut addend = AsyncTwoAwaitAddend::default();
    if !collect_two_addend(expr, first_name, second_name, &mut addend) {
        return None;
    }
    if !addend.has_first || !addend.has_second {
        return None;
    }
    if addend.first_coeff == 0 || addend.second_coeff == 0 {
        return None;
    }
    Some(AsyncTwoAwaitScalarPlan {
        first_coeff: addend.first_coeff,
        second_coeff: addend.second_coeff,
        immediate: addend.immediate,
    })
}

pub fn plan_async_two_await_continuation<'a>(
    func: &'a FuncDecl,
) -> Option<AsyncTwoAwaitContinuationPlan<'a>> {
    if !func.is_async {
        return None;
    }
    let first = async_await_binding_at(func, 0)?;
    let second = async_await_binding_at(func, first.next_idx)?;
    if second.next_idx + 1 != func.body.len() {
        return None;
    }
    let ret_expr = async_continuation_return_expr(&func.body[second.next_idx])?;
    let first_call = match first.state_expr {
        Node::CallExpr(c) => plan_future_runtime_call(c)?,
        _ => return None,
    };
    let second_call = match second.state_expr {
        Node::CallExpr(c) => plan_future_runtime_call(c)?,
        _ => return None,
    };
    if first_call.kind != FutureRuntimeCallKind::DeferReady
        || second_call.kind != FutureRuntimeCallKind::DeferReady
    {
        return None;
    }
    let scalar = async_two_await_scalar_expr(ret_expr, &first.binding_name, &second.binding_name)?;
    Some(AsyncTwoAwaitContinuationPlan {
        first_binding_name: first.binding_name,
        second_binding_name: second.binding_name,
        first_await_expr: first.state_expr,
        second_await_expr: second.state_expr,
        first_awaited_kind: first_call.kind,
        second_awaited_kind: second_call.kind,
        scalar,
    })
}

// -- linear await (lowering_rules.zig:2154-2265) ----------------------------

pub const ASYNC_LINEAR_AWAIT_MAX: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AsyncLinearAwaitScalarPlan {
    pub coeffs: [i64; ASYNC_LINEAR_AWAIT_MAX],
    pub immediate: i64,
}

impl Default for AsyncLinearAwaitScalarPlan {
    fn default() -> Self {
        AsyncLinearAwaitScalarPlan {
            coeffs: [0; ASYNC_LINEAR_AWAIT_MAX],
            immediate: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncLinearAwaitContinuationPlan<'a> {
    pub count: usize,
    pub binding_names: Vec<String>,
    pub await_exprs: Vec<&'a Node>,
    pub awaited_kinds: Vec<FutureRuntimeCallKind>,
    pub scalar: AsyncLinearAwaitScalarPlan,
}

impl<'a> AsyncLinearAwaitContinuationPlan<'a> {
    pub fn async_state_size(&self) -> usize {
        8 + 24 * self.count
    }
    pub fn value_offset(&self, index: usize) -> usize {
        8 + 16 * self.count + 8 * index
    }
    pub fn sub_state_offset(&self, index: usize) -> usize {
        8 + 16 * index
    }
}

#[derive(Debug, Clone)]
struct AsyncLinearAwaitAddend {
    used: [bool; ASYNC_LINEAR_AWAIT_MAX],
    coeffs: [i64; ASYNC_LINEAR_AWAIT_MAX],
    immediate: i64,
}

impl Default for AsyncLinearAwaitAddend {
    fn default() -> Self {
        AsyncLinearAwaitAddend {
            used: [false; ASYNC_LINEAR_AWAIT_MAX],
            coeffs: [0; ASYNC_LINEAR_AWAIT_MAX],
            immediate: 0,
        }
    }
}

fn scale_linear_addend(addend: &mut AsyncLinearAwaitAddend, count: usize, factor: i64) -> bool {
    for coeff in addend.coeffs[..count].iter_mut() {
        *coeff = match coeff.checked_mul(factor) {
            Some(v) => v,
            None => return false,
        };
    }
    addend.immediate = match addend.immediate.checked_mul(factor) {
        Some(v) => v,
        None => return false,
    };
    true
}

fn add_linear_addend(
    target: &mut AsyncLinearAwaitAddend,
    count: usize,
    addend: &AsyncLinearAwaitAddend,
) -> bool {
    for i in 0..count {
        if addend.used[i] {
            target.used[i] = true;
        }
        target.coeffs[i] = match target.coeffs[i].checked_add(addend.coeffs[i]) {
            Some(v) => v,
            None => return false,
        };
    }
    target.immediate = match target.immediate.checked_add(addend.immediate) {
        Some(v) => v,
        None => return false,
    };
    true
}

fn collect_linear_addend(
    expr: &Node,
    names: &[String],
    addend: &mut AsyncLinearAwaitAddend,
) -> bool {
    match expr {
        Node::Identifier(name) => {
            for (i, binding_name) in names.iter().enumerate() {
                if name == binding_name {
                    addend.used[i] = true;
                    addend.coeffs[i] = match addend.coeffs[i].checked_add(1) {
                        Some(v) => v,
                        None => return false,
                    };
                    return true;
                }
            }
            false
        }
        Node::Literal(Literal::IntVal(v)) => {
            addend.immediate = match addend.immediate.checked_add(*v) {
                Some(x) => x,
                None => return false,
            };
            true
        }
        Node::Literal(_) => false,
        Node::BinaryExpr(bin) => {
            if bin.op == BinaryOp::Mul {
                let left_lit = int_literal(&bin.left);
                let right_lit = int_literal(&bin.right);
                if left_lit.is_some() && right_lit.is_some() {
                    return false;
                }
                let factor = match left_lit.or(right_lit) {
                    Some(f) => f,
                    None => return false,
                };
                let expr_side = if left_lit.is_some() { &bin.right } else { &bin.left };
                let mut nested = AsyncLinearAwaitAddend::default();
                if !collect_linear_addend(expr_side, names, &mut nested) {
                    return false;
                }
                if !scale_linear_addend(&mut nested, names.len(), factor) {
                    return false;
                }
                return add_linear_addend(addend, names.len(), &nested);
            }
            if bin.op != BinaryOp::Add && bin.op != BinaryOp::Sub {
                return false;
            }
            let mut left = AsyncLinearAwaitAddend::default();
            let mut right = AsyncLinearAwaitAddend::default();
            if !collect_linear_addend(&bin.left, names, &mut left) {
                return false;
            }
            if !collect_linear_addend(&bin.right, names, &mut right) {
                return false;
            }
            if bin.op == BinaryOp::Sub && !scale_linear_addend(&mut right, names.len(), -1) {
                return false;
            }
            add_linear_addend(addend, names.len(), &left)
                && add_linear_addend(addend, names.len(), &right)
        }
        _ => false,
    }
}

pub fn async_linear_await_scalar_expr(
    expr: &Node,
    names: &[String],
) -> Option<AsyncLinearAwaitScalarPlan> {
    let mut addend = AsyncLinearAwaitAddend::default();
    if !collect_linear_addend(expr, names, &mut addend) {
        return None;
    }
    if !addend.used[..names.len()].iter().any(|u| *u) {
        return None;
    }
    let mut scalar = AsyncLinearAwaitScalarPlan::default();
    for i in 0..names.len() {
        scalar.coeffs[i] = addend.coeffs[i];
    }
    scalar.immediate = addend.immediate;
    Some(scalar)
}

pub fn plan_async_linear_await_continuation<'a>(
    func: &'a FuncDecl,
) -> Option<AsyncLinearAwaitContinuationPlan<'a>> {
    if !func.is_async {
        return None;
    }
    let mut binding_names: Vec<String> = Vec::new();
    let mut await_exprs: Vec<&Node> = Vec::new();
    let mut awaited_kinds: Vec<FutureRuntimeCallKind> = Vec::new();
    let mut idx = 0usize;
    while idx < func.body.len() && binding_names.len() < ASYNC_LINEAR_AWAIT_MAX {
        let binding = match async_await_binding_at(func, idx) {
            Some(b) => b,
            None => break,
        };
        let awaited_call = match binding.state_expr {
            Node::CallExpr(c) => plan_future_runtime_call(c)?,
            _ => return None,
        };
        if awaited_call.kind != FutureRuntimeCallKind::DeferReady {
            return None;
        }
        binding_names.push(binding.binding_name);
        await_exprs.push(binding.state_expr);
        awaited_kinds.push(awaited_call.kind);
        idx = binding.next_idx;
    }
    if binding_names.len() < 3 {
        return None;
    }
    if idx + 1 != func.body.len() {
        return None;
    }
    let ret_expr = async_continuation_return_expr(&func.body[idx])?;
    let scalar = async_linear_await_scalar_expr(ret_expr, &binding_names)?;
    Some(AsyncLinearAwaitContinuationPlan {
        count: binding_names.len(),
        binding_names,
        await_exprs,
        awaited_kinds,
        scalar,
    })
}

// -- join2 await (lowering_rules.zig:1927-2130) ------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AsyncPairResultScalarPlan {
    pub left_coeff: i64,
    pub right_coeff: i64,
    pub immediate: i64,
}

impl Default for AsyncPairResultScalarPlan {
    fn default() -> Self {
        AsyncPairResultScalarPlan {
            left_coeff: 1,
            right_coeff: 1,
            immediate: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncJoin2AwaitContinuationPlan<'a> {
    pub binding_name: String,
    pub await_expr: &'a Node,
    pub awaited_kind: FutureRuntimeCallKind,
    pub scalar: AsyncPairResultScalarPlan,
    pub prefix_len: usize,
}

impl<'a> AsyncJoin2AwaitContinuationPlan<'a> {
    pub fn async_state_size(&self) -> usize {
        16
    }
}

#[derive(Debug, Clone, Default)]
struct AsyncPairResultAddend {
    has_left: bool,
    has_right: bool,
    left_coeff: i64,
    right_coeff: i64,
    immediate: i64,
}

fn scale_pair_addend(addend: &mut AsyncPairResultAddend, factor: i64) -> bool {
    for v in [&mut addend.left_coeff, &mut addend.right_coeff, &mut addend.immediate] {
        *v = match v.checked_mul(factor) {
            Some(x) => x,
            None => return false,
        };
    }
    true
}

fn add_pair_addend(target: &mut AsyncPairResultAddend, addend: &AsyncPairResultAddend) -> bool {
    if addend.has_left {
        target.has_left = true;
    }
    if addend.has_right {
        target.has_right = true;
    }
    for (dst, src) in [
        (&mut target.left_coeff, addend.left_coeff),
        (&mut target.right_coeff, addend.right_coeff),
        (&mut target.immediate, addend.immediate),
    ] {
        *dst = match dst.checked_add(src) {
            Some(v) => v,
            None => return false,
        };
    }
    true
}

fn pair_accessor_kind(expr: &Node, binding_name: &str) -> Option<FutureRuntimeCallKind> {
    let call = match expr {
        Node::CallExpr(c) => c,
        _ => return None,
    };
    let plan = plan_future_runtime_call(call)?;
    if plan.kind != FutureRuntimeCallKind::PairLeft && plan.kind != FutureRuntimeCallKind::PairRight {
        return None;
    }
    if call.args.len() != 1 || !call.generics.is_empty() {
        return None;
    }
    match &call.args[0] {
        Node::Identifier(name) if name == binding_name => Some(plan.kind),
        _ => None,
    }
}

/// lowering_rules.zig:2007 `resolveLocalValue`.
fn resolve_local_value<'a>(func: &'a FuncDecl, upto_idx: usize, name: &str) -> Option<&'a Node> {
    let bound = upto_idx.min(func.body.len());
    let mut idx = bound;
    while idx > 0 {
        idx -= 1;
        let stmt = &func.body[idx];
        let let_stmt = match stmt {
            Node::LetStmt(l) => l,
            _ => continue,
        };
        if let_stmt.name != name {
            continue;
        }
        return Some(&let_stmt.value);
    }
    None
}

fn collect_pair_addend_ctx(
    expr: &Node,
    binding_name: &str,
    func: Option<&FuncDecl>,
    upto_idx: usize,
    depth: u8,
    addend: &mut AsyncPairResultAddend,
) -> bool {
    if let Some(kind) = pair_accessor_kind(expr, binding_name) {
        match kind {
            FutureRuntimeCallKind::PairLeft => {
                addend.has_left = true;
                addend.left_coeff = match addend.left_coeff.checked_add(1) {
                    Some(v) => v,
                    None => return false,
                };
            }
            FutureRuntimeCallKind::PairRight => {
                addend.has_right = true;
                addend.right_coeff = match addend.right_coeff.checked_add(1) {
                    Some(v) => v,
                    None => return false,
                };
            }
            _ => unreachable!(),
        }
        return true;
    }
    if let Node::Identifier(name) = expr {
        if depth >= 8 {
            return false;
        }
        let resolved = func.and_then(|f| resolve_local_value(f, upto_idx, name));
        let target = match resolved {
            Some(t) => t,
            None => return false,
        };
        if std::ptr::eq(target, expr) {
            return false;
        }
        return collect_pair_addend_ctx(target, binding_name, func, upto_idx, depth + 1, addend);
    }
    match expr {
        Node::Literal(Literal::IntVal(v)) => {
            addend.immediate = match addend.immediate.checked_add(*v) {
                Some(x) => x,
                None => return false,
            };
            true
        }
        Node::BinaryExpr(bin) => {
            if bin.op == BinaryOp::Mul {
                let left_lit = int_literal(&bin.left);
                let right_lit = int_literal(&bin.right);
                if left_lit.is_some() && right_lit.is_some() {
                    return false;
                }
                let factor = match left_lit.or(right_lit) {
                    Some(f) => f,
                    None => return false,
                };
                let expr_side = if left_lit.is_some() { &bin.right } else { &bin.left };
                let mut nested = AsyncPairResultAddend::default();
                if !collect_pair_addend_ctx(expr_side, binding_name, func, upto_idx, depth, &mut nested) {
                    return false;
                }
                if !scale_pair_addend(&mut nested, factor) {
                    return false;
                }
                return add_pair_addend(addend, &nested);
            }
            if bin.op != BinaryOp::Add && bin.op != BinaryOp::Sub {
                return false;
            }
            let mut left = AsyncPairResultAddend::default();
            let mut right = AsyncPairResultAddend::default();
            if !collect_pair_addend_ctx(&bin.left, binding_name, func, upto_idx, depth, &mut left) {
                return false;
            }
            if !collect_pair_addend_ctx(&bin.right, binding_name, func, upto_idx, depth, &mut right) {
                return false;
            }
            if bin.op == BinaryOp::Sub && !scale_pair_addend(&mut right, -1) {
                return false;
            }
            add_pair_addend(addend, &left) && add_pair_addend(addend, &right)
        }
        _ => false,
    }
}

pub fn async_pair_result_scalar_expr_ctx(
    expr: &Node,
    binding_name: &str,
    func: Option<&FuncDecl>,
    upto_idx: usize,
) -> Option<AsyncPairResultScalarPlan> {
    let mut addend = AsyncPairResultAddend::default();
    if !collect_pair_addend_ctx(expr, binding_name, func, upto_idx, 0, &mut addend) {
        return None;
    }
    if !addend.has_left && !addend.has_right {
        return None;
    }
    if addend.left_coeff == 0 && addend.right_coeff == 0 {
        return None;
    }
    Some(AsyncPairResultScalarPlan {
        left_coeff: addend.left_coeff,
        right_coeff: addend.right_coeff,
        immediate: addend.immediate,
    })
}

fn join2_arg_kind(func: &FuncDecl, upto_idx: usize, arg: &Node) -> Option<FutureRuntimeCallKind> {
    if let Node::CallExpr(c) = arg {
        let plan = plan_future_runtime_call(c)?;
        return Some(plan.kind);
    }
    let name = match arg {
        Node::Identifier(n) => n,
        _ => return None,
    };
    let mut idx = upto_idx;
    while idx > 0 {
        idx -= 1;
        let stmt = &func.body[idx];
        let let_stmt = match stmt {
            Node::LetStmt(l) => l,
            _ => continue,
        };
        if let_stmt.name != *name {
            continue;
        }
        return match &*let_stmt.value {
            Node::CallExpr(c) => plan_future_runtime_call(c).map(|p| p.kind),
            _ => None,
        };
    }
    None
}

fn join2_has_later_ready_input(func: &FuncDecl, upto_idx: usize, call: &sla_ast::CallExpr) -> bool {
    if call.args.len() != 2 {
        return false;
    }
    for arg in &call.args {
        let kind = match join2_arg_kind(func, upto_idx, arg) {
            Some(k) => k,
            None => return false,
        };
        match kind {
            FutureRuntimeCallKind::Ready
            | FutureRuntimeCallKind::DeferReady
            | FutureRuntimeCallKind::Pending => {}
            _ => return false,
        }
    }
    true
}

pub fn plan_async_join2_await_continuation<'a>(
    func: &'a FuncDecl,
) -> Option<AsyncJoin2AwaitContinuationPlan<'a>> {
    if !func.is_async {
        return None;
    }
    let mut await_idx = 0usize;
    let mut awaited: Option<AsyncAwaitBindingShape<'a>> = None;
    while await_idx < func.body.len() {
        if let Some(shape) = async_await_binding_at(func, await_idx) {
            awaited = Some(shape);
            break;
        }
        if !matches!(&func.body[await_idx], Node::LetStmt(_)) {
            return None;
        }
        await_idx += 1;
    }
    let awaited = awaited?;
    if func.body.len() < 2 {
        return None;
    }
    let ret_idx = func.body.len() - 1;
    if ret_idx < awaited.next_idx {
        return None;
    }
    let mut tail_idx = awaited.next_idx;
    while tail_idx < ret_idx {
        if !matches!(&func.body[tail_idx], Node::LetStmt(_)) {
            return None;
        }
        tail_idx += 1;
    }
    let ret_expr = async_continuation_return_expr(&func.body[ret_idx])?;
    let awaited_call_expr = match awaited.state_expr {
        Node::CallExpr(c) => c,
        _ => return None,
    };
    let awaited_call = plan_future_runtime_call(awaited_call_expr)?;
    if awaited_call.kind != FutureRuntimeCallKind::Join2 {
        return None;
    }
    if !join2_has_later_ready_input(func, await_idx, awaited_call_expr) {
        return None;
    }
    let scalar = async_pair_result_scalar_expr_ctx(
        ret_expr,
        &awaited.binding_name,
        Some(func),
        func.body.len(),
    )?;
    Some(AsyncJoin2AwaitContinuationPlan {
        binding_name: awaited.binding_name.clone(),
        await_expr: awaited.state_expr,
        awaited_kind: awaited_call.kind,
        scalar,
        prefix_len: await_idx,
    })
}
