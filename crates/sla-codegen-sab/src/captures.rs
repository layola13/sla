//! E6 — escaped-closure capture collection and local
//! queries, mirroring `sab_codegen.zig` lines 4039–4239 in
//! the reference tree (`/content/sa_all`, HEAD 2cc12ea8):
//! `collectEscapedCaptures*`, `addEscapedCapture`,
//! `callNameResolvesToValue`, and the `localReg` /
//! `localType` / `stackLocal` / `isLocalReg` / `isParamReg`
//! queries the whole crate consults. (Decomposition doc
//! lines 3378–3577 in the older tree.)
//!
//! Faithfulness notes:
//! - The walkers reproduce Zig's arm lists exactly: node
//!   shapes Zig's `else => {}` skips (slice/try/await/enum
//!   literals in the expr walker, most statements in the
//!   block walker) are skipped here too — widening the walk
//!   would be an improvement, and improvements are out of
//!   discipline for this port.
//! - Zig keys "is this name taken" by `tc.funcs` /
//!   `tc.macros` membership; the port asks the same questions
//!   through `CheckInfo::func` / `CheckInfo::is_macro`.
//! - Capture offsets are Zig's: `16 + index * 8` (the
//!   closure object header is two words).

use std::collections::HashSet;

use sla_ast::{ClosureLiteral, Node, Type};
use sla_lowering::{is_internal_symbol, CheckInfo};

use crate::error::{Error, Result};
use crate::layout::user_defined_generic_inner;
use crate::state::CodegenState;
use crate::types::{EscapedCapture, EscapedCaptureCollector, Local};

/// Zig `localReg`: newest binding wins. (Also on
/// `FrameState` since B3; this is the E6 entry point Zig's
/// call sites use.)
pub fn local_reg(state: &CodegenState, name: &str) -> Option<u32> {
    state.frame.local_reg(name)
}

/// Zig `localRegIsAddress`: the named local's register holds
/// an address (stack slot), not value bits.
pub fn local_reg_is_address(state: &CodegenState, name: &str) -> bool {
    for local in state.frame.locals.iter().rev() {
        if local.name == name {
            return local.is_stack_alloc || local.stack_ty.is_some();
        }
    }
    false
}

/// Zig `localType`: the declared type, falling back to the
/// stack-slot type.
pub fn local_type(state: &CodegenState, name: &str) -> Option<Type> {
    for local in state.frame.locals.iter().rev() {
        if local.name == name {
            return local.ty.clone().or_else(|| local.stack_ty.clone());
        }
    }
    None
}

/// Zig `stackLocal`: the newest local with a stack-slot type.
pub fn stack_local(state: &CodegenState, name: &str) -> Option<Local> {
    for local in state.frame.locals.iter().rev() {
        if local.name == name && local.stack_ty.is_some() {
            return Some(local.clone());
        }
    }
    None
}

/// Zig `isLocalReg`.
pub fn is_local_reg(state: &CodegenState, reg: u32) -> bool {
    state.frame.locals.iter().any(|local| local.reg == reg)
}

/// Zig `isParamReg`.
pub fn is_param_reg(state: &CodegenState, reg: u32) -> bool {
    for local in &state.frame.locals {
        if local.reg == reg {
            return local.is_param;
        }
    }
    false
}

/// Zig `closureLiteralFromExpr`: a closure literal, possibly
/// under a move expression.
pub fn closure_literal_from_expr(expr: &Node) -> Option<&ClosureLiteral> {
    match expr {
        Node::ClosureLiteral(lit) => Some(lit),
        Node::MoveExpr(mv) => closure_literal_from_expr(&mv.expr),
        _ => None,
    }
}

/// Zig `joinHandleInnerType` (lowering_rules.zig:4868).
pub fn join_handle_inner_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "JoinHandle")
}

/// Zig `addEscapedCapture`: record `name` as a capture
/// unless it is shadowed by a walker-local, is a function or
/// macro name, is an internal symbol, or was already
/// collected. The capture type comes from the call site or
/// the local table; neither is a loud `MissingType`.
pub fn add_escaped_capture(
    state: &CodegenState,
    check: &dyn CheckInfo,
    name: &str,
    ty: Option<&Type>,
    captures: &mut EscapedCaptureCollector,
    locals: &HashSet<String>,
) -> Result<()> {
    if locals.contains(name) {
        return Ok(());
    }
    if check.func(name).is_some() {
        return Ok(());
    }
    if check.is_macro(name) {
        return Ok(());
    }
    if is_internal_symbol(name) {
        return Ok(());
    }
    if captures.seen.contains(name) {
        return Ok(());
    }
    let capture_ty = match ty {
        Some(ty) => ty.clone(),
        None => local_type(state, name).ok_or_else(|| {
            Error::MissingType(format!(
                "addEscapedCapture: no type for capture `{name}` [group E6 captures]"
            ))
        })?,
    };
    let offset = 16 + captures.ordered.len() * 8;
    captures.ordered.push(EscapedCapture {
        name: name.to_string(),
        offset,
        ty: capture_ty,
    });
    captures.seen.insert(name.to_string());
    Ok(())
}

/// Zig `collectEscapedCapturesInBlock`.
pub fn collect_escaped_captures_in_block(
    state: &CodegenState,
    check: &dyn CheckInfo,
    block: &[Node],
    captures: &mut EscapedCaptureCollector,
    locals: &mut HashSet<String>,
) -> Result<()> {
    for stmt in block {
        match stmt {
            Node::LetStmt(let_stmt) => {
                collect_escaped_captures_in_expr(state, check, &let_stmt.value, captures, locals)?;
                locals.insert(let_stmt.name.clone());
            }
            Node::LetDestructureStmt(let_stmt) => {
                collect_escaped_captures_in_expr(state, check, &let_stmt.value, captures, locals)?;
                for name in &let_stmt.names {
                    locals.insert(name.clone());
                }
            }
            Node::VarStmt(v) => {
                locals.insert(v.name.clone());
            }
            Node::ConstStmt(c) => {
                collect_escaped_captures_in_expr(state, check, &c.value, captures, locals)?;
                locals.insert(c.name.clone());
            }
            Node::AssignStmt(assign) => {
                collect_escaped_captures_in_expr(state, check, &assign.target, captures, locals)?;
                collect_escaped_captures_in_expr(state, check, &assign.value, captures, locals)?;
            }
            Node::BlockStmt(blk) => {
                let mut child = locals.clone();
                collect_escaped_captures_in_block(state, check, &blk.body, captures, &mut child)?;
            }
            Node::ArenaStmt(blk) => {
                let mut child = locals.clone();
                collect_escaped_captures_in_block(state, check, &blk.body, captures, &mut child)?;
            }
            Node::ExprStmt(expr) => {
                collect_escaped_captures_in_expr(state, check, expr, captures, locals)?;
            }
            Node::ReturnStmt(ret) => {
                if let Some(value) = &ret.value {
                    collect_escaped_captures_in_expr(state, check, value, captures, locals)?;
                }
            }
            Node::ForStmt(f) => {
                collect_escaped_captures_in_expr(state, check, &f.start, captures, locals)?;
                if let Some(end) = &f.end {
                    collect_escaped_captures_in_expr(state, check, end, captures, locals)?;
                }
                let mut child = locals.clone();
                child.insert(f.var_name.clone());
                collect_escaped_captures_in_block(state, check, &f.body, captures, &mut child)?;
            }
            Node::WhileStmt(w) => {
                collect_escaped_captures_in_expr(state, check, &w.cond, captures, locals)?;
                let mut child = locals.clone();
                collect_escaped_captures_in_block(state, check, &w.body, captures, &mut child)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Zig `collectEscapedCapturesInExpr`.
pub fn collect_escaped_captures_in_expr(
    state: &CodegenState,
    check: &dyn CheckInfo,
    expr: &Node,
    captures: &mut EscapedCaptureCollector,
    locals: &mut HashSet<String>,
) -> Result<()> {
    match expr {
        Node::Identifier(name) => {
            let ty = check.expr_type(expr);
            add_escaped_capture(state, check, name, ty, captures, locals)?;
        }
        Node::BinaryExpr(bin) => {
            collect_escaped_captures_in_expr(state, check, &bin.left, captures, locals)?;
            collect_escaped_captures_in_expr(state, check, &bin.right, captures, locals)?;
        }
        Node::BorrowExpr(b) => {
            collect_escaped_captures_in_expr(state, check, &b.expr, captures, locals)?;
        }
        Node::MoveExpr(m) => {
            collect_escaped_captures_in_expr(state, check, &m.expr, captures, locals)?;
        }
        Node::DerefExpr(d) => {
            collect_escaped_captures_in_expr(state, check, &d.expr, captures, locals)?;
        }
        Node::CastExpr(c) => {
            collect_escaped_captures_in_expr(state, check, &c.expr, captures, locals)?;
        }
        Node::FieldExpr(f) => {
            collect_escaped_captures_in_expr(state, check, &f.expr, captures, locals)?;
        }
        Node::CallExpr(call) => {
            if call.associated_target.is_none()
                && call.callee.is_none()
                && call_name_resolves_to_value(state, check, &call.func_name)
            {
                let ty = local_type(state, &call.func_name);
                add_escaped_capture(state, check, &call.func_name, ty.as_ref(), captures, locals)?;
            }
            if let Some(callee) = &call.callee {
                collect_escaped_captures_in_expr(state, check, callee, captures, locals)?;
            }
            for arg in &call.args {
                collect_escaped_captures_in_expr(state, check, arg, captures, locals)?;
            }
        }
        Node::StructLiteral(lit) => {
            for field in &lit.fields {
                collect_escaped_captures_in_expr(state, check, &field.value, captures, locals)?;
            }
            if let Some(update) = &lit.update_expr {
                collect_escaped_captures_in_expr(state, check, update, captures, locals)?;
            }
        }
        Node::TupleLiteral(lit) => {
            for elem in &lit.elements {
                collect_escaped_captures_in_expr(state, check, elem, captures, locals)?;
            }
        }
        Node::ArrayLiteral(lit) => {
            for elem in &lit.elements {
                collect_escaped_captures_in_expr(state, check, elem, captures, locals)?;
            }
        }
        Node::RepeatArrayLiteral(lit) => {
            collect_escaped_captures_in_expr(state, check, &lit.value, captures, locals)?;
        }
        Node::IndexExpr(idx) => {
            collect_escaped_captures_in_expr(state, check, &idx.target, captures, locals)?;
            collect_escaped_captures_in_expr(state, check, &idx.index, captures, locals)?;
        }
        Node::IfExpr(ife) => {
            collect_escaped_captures_in_expr(state, check, &ife.cond, captures, locals)?;
            let mut then_locals = locals.clone();
            collect_escaped_captures_in_block(state, check, &ife.then_block, captures, &mut then_locals)?;
            if let Some(else_block) = &ife.else_block {
                let mut else_locals = locals.clone();
                collect_escaped_captures_in_block(state, check, else_block, captures, &mut else_locals)?;
            }
        }
        Node::ClosureLiteral(_) => {}
        _ => {}
    }
    Ok(())
}

/// Zig `callNameResolvesToValue`: a bare call name denotes a
/// value in scope — a free function, a macro, an internal
/// symbol, or an in-scope local (an fn-ptr variable).
pub fn call_name_resolves_to_value(state: &CodegenState, check: &dyn CheckInfo, name: &str) -> bool {
    if check.func(name).is_some() {
        return true;
    }
    if check.is_macro(name) {
        return true;
    }
    if is_internal_symbol(name) {
        return true;
    }
    local_reg(state, name).is_some()
}

/// Zig `collectEscapedClosureCaptures`: walk a closure body
/// with its params as the initial walker-locals.
pub fn collect_escaped_closure_captures(
    state: &CodegenState,
    check: &dyn CheckInfo,
    closure: &ClosureLiteral,
) -> Result<Vec<EscapedCapture>> {
    let mut locals: HashSet<String> = HashSet::new();
    for param in &closure.params {
        locals.insert(param.name.clone());
    }
    let mut collector = EscapedCaptureCollector::new();
    collect_escaped_captures_in_expr(state, check, &closure.body, &mut collector, &mut locals)?;
    Ok(collector.ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{BinaryExpr, BinaryOp, Literal, Param, Primitive};
    use sla_lowering::MapCheckInfo;

    fn ident(name: &str) -> Node {
        Node::Identifier(name.to_string())
    }

    fn i32_ty() -> Type {
        Type::Primitive(Primitive::I32)
    }

    #[test]
    fn local_queries_newest_wins() {
        let mut st = CodegenState::new();
        crate::locals::push_local(&mut st, "x", 1, false).unwrap();
        crate::locals::push_typed_local(&mut st, "x", 2, true, &i32_ty()).unwrap();
        assert_eq!(local_reg(&st, "x"), Some(2));
        assert!(is_param_reg(&st, 2));
        assert!(!is_param_reg(&st, 1));
        assert_eq!(local_type(&st, "x"), Some(i32_ty()));
        assert!(stack_local(&st, "x").is_none());
    }

    #[test]
    fn captures_collect_outer_names_with_offsets() {
        let mut st = CodegenState::new();
        crate::locals::push_typed_local(&mut st, "a", 1, false, &i32_ty()).unwrap();
        crate::locals::push_typed_local(&mut st, "b", 2, false, &i32_ty()).unwrap();
        let check = MapCheckInfo::new();
        let body = Node::BinaryExpr(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(ident("a")),
            right: Box::new(ident("b")),
        });
        let closure = ClosureLiteral {
            params: vec![Param {
                is_var: false,
                name: "a".to_string(),
                ty: Box::new(i32_ty()),
                is_borrow: false,
                is_move: false,
                fallible_sugar: false,
            }],
            body: Box::new(body),
        };
        let caps = collect_escaped_closure_captures(&st, &check, &closure).unwrap();
        // `a` is a param (walker-local), only `b` is captured.
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].name, "b");
        assert_eq!(caps[0].offset, 16);
    }

    #[test]
    fn capture_without_type_is_missing_type() {
        let st = CodegenState::new();
        let check = MapCheckInfo::new();
        let closure = ClosureLiteral {
            params: vec![],
            body: Box::new(ident("ghost")),
        };
        match collect_escaped_closure_captures(&st, &check, &closure) {
            Err(Error::MissingType(_)) => {}
            other => panic!("expected MissingType, got {other:?}"),
        }
    }

    #[test]
    fn closure_literal_peels_move() {
        let lit = ClosureLiteral {
            params: vec![],
            body: Box::new(Node::Literal(Literal::IntVal(0))),
        };
        let node = Node::MoveExpr(sla_ast::MoveExpr {
            expr: Box::new(Node::ClosureLiteral(lit.clone())),
        });
        assert!(closure_literal_from_expr(&node).is_some());
        assert!(closure_literal_from_expr(&ident("x")).is_none());
        let _ = lit;
    }
}
