//! Async general + loop continuation planners, mirrored from
//! lowering_rules.zig: planAsyncGeneralAwaitContinuation (2339) and
//! planAsyncLoopAwaitContinuation (2582) with their scope/whitelist
//! checks. Stage 6 async cluster.

use sla_ast::{FuncDecl, Literal, Node, Primitive, Type};

use crate::async_plans::{
    async_await_binding_at, async_continuation_return_expr, async_linear_await_scalar_expr,
    stmt_is_prebound_await_state, AsyncLinearAwaitScalarPlan,
};
use crate::async_runtime::{plan_future_runtime_call, FutureRuntimeCallKind};

// -- general await (lowering_rules.zig:2263-2400) --------------------------

pub const ASYNC_GENERAL_AWAIT_MAX: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncGeneralAwaitContinuationPlan<'a> {
    pub count: usize,
    pub binding_names: Vec<String>,
    pub await_exprs: Vec<&'a Node>,
    pub scalar: AsyncLinearAwaitScalarPlan,
    pub prefix_len: usize,
}

impl<'a> AsyncGeneralAwaitContinuationPlan<'a> {
    pub fn async_state_size(&self) -> usize {
        8 + 16 * self.count
    }
    pub fn object_offset(&self, _index: usize) -> usize {
        8 + 8 * _index
    }
    pub fn value_offset(&self, index: usize) -> usize {
        8 + 8 * self.count + 8 * index
    }
}

/// lowering_rules.zig:2305 `generalAwaitExprInScope`.
fn general_await_expr_in_scope(expr: &Node, allowed: &[String]) -> bool {
    match expr {
        Node::Identifier(name) => allowed.iter().any(|a| a == name),
        Node::Literal(_) => true,
        Node::BinaryExpr(bin) => {
            general_await_expr_in_scope(&bin.left, allowed)
                && general_await_expr_in_scope(&bin.right, allowed)
        }
        Node::CallExpr(call) => call
            .args
            .iter()
            .all(|arg| general_await_expr_in_scope(arg, allowed)),
        Node::FieldExpr(field) => general_await_expr_in_scope(&field.expr, allowed),
        Node::IndexExpr(idx) => {
            general_await_expr_in_scope(&idx.target, allowed)
                && general_await_expr_in_scope(&idx.index, allowed)
        }
        Node::CastExpr(cast) => general_await_expr_in_scope(&cast.expr, allowed),
        Node::DerefExpr(deref) => general_await_expr_in_scope(&deref.expr, allowed),
        Node::BorrowExpr(borrow) => general_await_expr_in_scope(&borrow.expr, allowed),
        Node::MoveExpr(mv) => general_await_expr_in_scope(&mv.expr, allowed),
        Node::TupleLiteral(lit) => lit
            .elements
            .iter()
            .all(|e| general_await_expr_in_scope(e, allowed)),
        Node::ArrayLiteral(lit) => lit
            .elements
            .iter()
            .all(|e| general_await_expr_in_scope(e, allowed)),
        Node::RepeatArrayLiteral(lit) => general_await_expr_in_scope(&lit.value, allowed),
        Node::StructLiteral(lit) => {
            lit.fields
                .iter()
                .all(|f| general_await_expr_in_scope(&f.value, allowed))
                && match &lit.update_expr {
                    Some(update) => general_await_expr_in_scope(update, allowed),
                    None => true,
                }
        }
        Node::EnumLiteral(lit) => lit
            .fields
            .iter()
            .all(|f| general_await_expr_in_scope(&f.value, allowed)),
        Node::SliceExpr(slc) => {
            general_await_expr_in_scope(&slc.target, allowed)
                && general_await_expr_in_scope(&slc.start, allowed)
                && general_await_expr_in_scope(&slc.end, allowed)
        }
        _ => false,
    }
}

pub fn plan_async_general_await_continuation<'a>(
    func: &'a FuncDecl,
) -> Option<AsyncGeneralAwaitContinuationPlan<'a>> {
    if !func.is_async {
        return None;
    }
    let mut binding_names: Vec<String> = Vec::new();
    let mut await_exprs: Vec<&Node> = Vec::new();
    let mut idx = 0usize;
    while idx < func.body.len() {
        let stmt = &func.body[idx];
        let let_stmt = match stmt {
            Node::LetStmt(l) => l,
            _ => return None,
        };
        let is_await = matches!(&*let_stmt.value, Node::AwaitExpr(_))
            || (idx + 1 < func.body.len()
                && stmt_is_prebound_await_state(&func.body[idx], &func.body[idx + 1]));
        if is_await {
            break;
        }
        idx += 1;
    }
    let prefix_len = idx;
    while idx < func.body.len() && binding_names.len() < ASYNC_GENERAL_AWAIT_MAX {
        let binding = match async_await_binding_at(func, idx) {
            Some(b) => b,
            None => break,
        };
        let call = match binding.state_expr {
            Node::CallExpr(c) => c,
            _ => return None,
        };
        if let Some(known) = plan_future_runtime_call(call) {
            if known.kind == FutureRuntimeCallKind::Join2
                || known.kind == FutureRuntimeCallKind::Select2
            {
                return None;
            }
        }
        if !binding_names.is_empty()
            && !general_await_expr_in_scope(binding.state_expr, &binding_names)
        {
            return None;
        }
        binding_names.push(binding.binding_name);
        await_exprs.push(binding.state_expr);
        idx = binding.next_idx;
    }
    if binding_names.len() < 2 {
        return None;
    }
    if idx + 1 != func.body.len() {
        return None;
    }
    let ret_expr = async_continuation_return_expr(&func.body[idx])?;
    let scalar = async_linear_await_scalar_expr(ret_expr, &binding_names)?;
    Some(AsyncGeneralAwaitContinuationPlan {
        count: binding_names.len(),
        binding_names,
        await_exprs,
        scalar,
        prefix_len,
    })
}

// -- loop await (lowering_rules.zig:2382-2693) -------------------------------

pub const ASYNC_LOOP_AWAIT_MAX_LOCALS: usize = 24;

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncLoopAwaitLocal {
    pub name: String,
    pub is_ptr: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AsyncLoopAwaitContinuationPlan<'a> {
    pub locals: Vec<AsyncLoopAwaitLocal>,
    pub head_local_count: usize,
    pub prefix_len: usize,
    pub binding_name: String,
    pub await_expr: &'a Node,
    pub cond_expr: &'a Node,
    pub ret_expr: &'a Node,
    pub pre_stmts: &'a [Node],
    pub post_stmts: &'a [Node],
    pub post_let_names: Vec<String>,
}

impl<'a> AsyncLoopAwaitContinuationPlan<'a> {
    pub fn local_offset(index: usize) -> usize {
        16 + 8 * index
    }
    pub fn sub_object_offset() -> usize {
        8
    }
    pub fn binding_index(&self) -> usize {
        self.locals.len() - 1
    }
    pub fn async_state_size(&self) -> usize {
        16 + 8 * self.locals.len()
    }
}

struct LoopAwaitBinding<'a> {
    binding_name: String,
    state_expr: &'a Node,
    start: usize,
    next: usize,
}

fn loop_await_binding_at<'a>(stmts: &'a [Node], idx: usize) -> Option<LoopAwaitBinding<'a>> {
    if idx >= stmts.len() {
        return None;
    }
    let await_let = match &stmts[idx] {
        Node::LetStmt(l) => l,
        _ => return None,
    };
    if let Node::AwaitExpr(a) = &*await_let.value {
        return Some(LoopAwaitBinding {
            binding_name: await_let.name.clone(),
            state_expr: &a.expr,
            start: idx,
            next: idx + 1,
        });
    }
    if idx + 1 >= stmts.len() || !stmt_is_prebound_await_state(&stmts[idx], &stmts[idx + 1]) {
        return None;
    }
    let state_let = match &stmts[idx] {
        Node::LetStmt(l) => l,
        _ => unreachable!(),
    };
    let bound_await_let = match &stmts[idx + 1] {
        Node::LetStmt(l) => l,
        _ => unreachable!(),
    };
    Some(LoopAwaitBinding {
        binding_name: bound_await_let.name.clone(),
        state_expr: &state_let.value,
        start: idx,
        next: idx + 2,
    })
}

/// lowering_rules.zig:2450 `asyncLoopExprOk`.
pub fn async_loop_expr_ok(node: &Node) -> bool {
    match node {
        Node::Literal(_) | Node::Identifier(_) => true,
        Node::BinaryExpr(bin) => async_loop_expr_ok(&bin.left) && async_loop_expr_ok(&bin.right),
        Node::CallExpr(call) => {
            if call.callee.is_some() {
                return false;
            }
            call.args.iter().all(|arg| async_loop_expr_ok(arg))
        }
        Node::FieldExpr(field) => async_loop_expr_ok(&field.expr),
        Node::IndexExpr(idx) => async_loop_expr_ok(&idx.target) && async_loop_expr_ok(&idx.index),
        Node::CastExpr(cast) => async_loop_expr_ok(&cast.expr),
        Node::DerefExpr(deref) => async_loop_expr_ok(&deref.expr),
        Node::BorrowExpr(borrow) => async_loop_expr_ok(&borrow.expr),
        Node::MoveExpr(mv) => async_loop_expr_ok(&mv.expr),
        Node::TupleLiteral(lit) => lit.elements.iter().all(|e| async_loop_expr_ok(e)),
        Node::ArrayLiteral(lit) => lit.elements.iter().all(|e| async_loop_expr_ok(e)),
        Node::RepeatArrayLiteral(lit) => async_loop_expr_ok(&lit.value),
        Node::StructLiteral(lit) => {
            lit.fields.iter().all(|f| async_loop_expr_ok(&f.value))
                && match &lit.update_expr {
                    Some(update) => async_loop_expr_ok(update),
                    None => true,
                }
        }
        Node::EnumLiteral(lit) => lit.fields.iter().all(|f| async_loop_expr_ok(&f.value)),
        Node::SliceExpr(slc) => {
            async_loop_expr_ok(&slc.target)
                && async_loop_expr_ok(&slc.start)
                && async_loop_expr_ok(&slc.end)
        }
        Node::IfExpr(ifx) => {
            if !async_loop_expr_ok(&ifx.cond) {
                return false;
            }
            if !ifx.then_block.iter().all(|s| async_loop_stmt_ok(s)) {
                return false;
            }
            if let Some(else_block) = &ifx.else_block {
                if !else_block.iter().all(|s| async_loop_stmt_ok(s)) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}

/// lowering_rules.zig:2490 `asyncLoopStmtOk`.
pub fn async_loop_stmt_ok(node: &Node) -> bool {
    match node {
        Node::LetStmt(let_stmt) => async_loop_expr_ok(&let_stmt.value),
        Node::ConstStmt(const_stmt) => async_loop_expr_ok(&const_stmt.value),
        Node::AssignStmt(assign) => {
            async_loop_expr_ok(&assign.target) && async_loop_expr_ok(&assign.value)
        }
        Node::ExprStmt(inner) => async_loop_expr_ok(inner),
        Node::BlockStmt(block) => block.body.iter().all(|s| async_loop_stmt_ok(s)),
        _ => false,
    }
}

fn collect_loop_let_names(node: &Node, names: &mut Vec<String>) {
    match node {
        Node::LetStmt(let_stmt) => {
            names.push(let_stmt.name.clone());
            collect_loop_let_names(&let_stmt.value, names);
        }
        Node::ConstStmt(const_stmt) => {
            names.push(const_stmt.name.clone());
            collect_loop_let_names(&const_stmt.value, names);
        }
        Node::AssignStmt(assign) => {
            collect_loop_let_names(&assign.target, names);
            collect_loop_let_names(&assign.value, names);
        }
        Node::ExprStmt(inner) => collect_loop_let_names(inner, names),
        Node::BlockStmt(block) => {
            for stmt in &block.body {
                collect_loop_let_names(stmt, names);
            }
        }
        Node::IfExpr(ifx) => {
            collect_loop_let_names(&ifx.cond, names);
            for stmt in &ifx.then_block {
                collect_loop_let_names(stmt, names);
            }
            if let Some(else_block) = &ifx.else_block {
                for stmt in else_block {
                    collect_loop_let_names(stmt, names);
                }
            }
        }
        Node::BinaryExpr(bin) => {
            collect_loop_let_names(&bin.left, names);
            collect_loop_let_names(&bin.right, names);
        }
        Node::CallExpr(call) => {
            for arg in &call.args {
                collect_loop_let_names(arg, names);
            }
        }
        _ => {}
    }
}

fn loop_type_is_ptr(ty: &Type) -> bool {
    match ty {
        Type::Primitive(prim) => *prim == Primitive::RawPtr,
        _ => true,
    }
}

fn loop_local_class(
    ty: Option<&Type>,
    value: &Node,
    prior: &[AsyncLoopAwaitLocal],
) -> Option<bool> {
    if let Some(declared) = ty {
        return Some(loop_type_is_ptr(declared));
    }
    match value {
        Node::Literal(lit) => Some(matches!(lit, Literal::StringVal(_))),
        Node::Identifier(name) => prior
            .iter()
            .find(|local| &local.name == name)
            .map(|local| local.is_ptr),
        _ => None,
    }
}

pub fn plan_async_loop_await_continuation<'a>(
    func: &'a FuncDecl,
) -> Option<AsyncLoopAwaitContinuationPlan<'a>> {
    if !func.is_async {
        return None;
    }
    if matches!(&*func.ret_ty, Type::Primitive(Primitive::VoidType)) {
        return None;
    }
    if func.body.len() < 2 {
        return None;
    }
    let ret_node = &func.body[func.body.len() - 1];
    let ret_expr = match ret_node {
        Node::ReturnStmt(ret) => ret.value.as_deref()?,
        _ => return None,
    };
    if !async_loop_expr_ok(ret_expr) {
        return None;
    }
    let while_node = &func.body[func.body.len() - 2];
    let while_stmt = match while_node {
        Node::WhileStmt(w) => w,
        _ => return None,
    };
    if while_stmt.let_pattern.is_some() {
        return None;
    }
    if !async_loop_expr_ok(&while_stmt.cond) {
        return None;
    }

    let prefix_len = func.body.len() - 2;
    let mut locals: Vec<AsyncLoopAwaitLocal> = Vec::new();

    for param in &func.params {
        if locals.len() >= ASYNC_LOOP_AWAIT_MAX_LOCALS {
            return None;
        }
        locals.push(AsyncLoopAwaitLocal {
            name: param.name.clone(),
            is_ptr: loop_type_is_ptr(&param.ty),
        });
    }
    for stmt in &func.body[..prefix_len] {
        let let_stmt = match stmt {
            Node::LetStmt(l) => l,
            _ => return None,
        };
        if !async_loop_expr_ok(&let_stmt.value) {
            return None;
        }
        if locals.len() >= ASYNC_LOOP_AWAIT_MAX_LOCALS {
            return None;
        }
        let is_ptr = loop_local_class(let_stmt.ty.as_deref(), &let_stmt.value, &locals)?;
        locals.push(AsyncLoopAwaitLocal {
            name: let_stmt.name.clone(),
            is_ptr,
        });
    }
    let head_local_count = locals.len();

    let mut binding: Option<LoopAwaitBinding> = None;
    let mut scan = 0usize;
    while scan < while_stmt.body.len() {
        if let Some(found) = loop_await_binding_at(&while_stmt.body, scan) {
            binding = Some(found);
            break;
        }
        scan += 1;
    }
    let bind = binding?;
    if !matches!(bind.state_expr, Node::CallExpr(_)) {
        return None;
    }
    if !async_loop_expr_ok(bind.state_expr) {
        return None;
    }
    let pre_stmts = &while_stmt.body[..bind.start];
    let post_stmts = &while_stmt.body[bind.next..];
    if !pre_stmts.iter().all(|s| async_loop_stmt_ok(s)) {
        return None;
    }
    if !post_stmts.iter().all(|s| async_loop_stmt_ok(s)) {
        return None;
    }

    let mut seg_names: Vec<String> = Vec::new();
    for stmt in pre_stmts {
        collect_loop_let_names(stmt, &mut seg_names);
    }
    for stmt in post_stmts {
        collect_loop_let_names(stmt, &mut seg_names);
    }
    for seg_name in &seg_names {
        for local in &locals {
            if &local.name == seg_name {
                return None;
            }
        }
        if *seg_name == bind.binding_name {
            return None;
        }
    }
    for local in &locals {
        if local.name == bind.binding_name {
            return None;
        }
    }

    for stmt in pre_stmts {
        if let Node::LetStmt(let_stmt) = stmt {
            if locals.len() >= ASYNC_LOOP_AWAIT_MAX_LOCALS {
                return None;
            }
            let is_ptr = loop_local_class(let_stmt.ty.as_deref(), &let_stmt.value, &locals)?;
            locals.push(AsyncLoopAwaitLocal {
                name: let_stmt.name.clone(),
                is_ptr,
            });
        }
    }
    if locals.len() >= ASYNC_LOOP_AWAIT_MAX_LOCALS {
        return None;
    }
    locals.push(AsyncLoopAwaitLocal {
        name: bind.binding_name.clone(),
        is_ptr: false,
    });

    let mut post_let_names: Vec<String> = Vec::new();
    for stmt in post_stmts {
        if let Node::LetStmt(let_stmt) = stmt {
            if post_let_names.len() >= 8 {
                return None;
            }
            post_let_names.push(let_stmt.name.clone());
        }
    }

    Some(AsyncLoopAwaitContinuationPlan {
        locals,
        head_local_count,
        prefix_len,
        binding_name: bind.binding_name,
        await_expr: bind.state_expr,
        cond_expr: &while_stmt.cond,
        ret_expr,
        pre_stmts,
        post_stmts,
        post_let_names,
    })
}
