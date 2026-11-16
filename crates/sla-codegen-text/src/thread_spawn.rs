//! `thread::spawn` associated-target calls (RUST-S7-SPAWN-ASSOC):
//! a port of the Zig text backend's thread-spawn helper machinery
//! (codegen.zig `threadSpawnHelperForExpr` /
//! `collectThreadSpawnHelpers` / `emitThreadSpawnHelpers` and the
//! `isThreadSpawnCall` call-site arm; lowering_rules.zig
//! `isThreadSpawnCall`, `joinHandleInnerType`,
//! `planEscapedClosureExecution`).
//!
//! Shape of the lowering, per call site (registered in a pre-pass
//! over the program, in source order):
//! - a slot block is allocated in the caller: `+0` = the i32
//!   thread handle (`-1` for an inline-joined spawn), `+8` = the
//!   closure's return value, captures follow at 8-byte offsets
//!   from 16;
//! - non-inline spawns call a per-site `@ffi_wrapper
//!   sla_thread_spawn_{idx}(*slot)` helper that runs
//!   `EXPAND THREAD_SPAWN` (sa_std/thread.sa) with the worker's
//!   vtable entry and stores the handle into `slot+0`;
//! - the worker `@sla_thread_worker_{idx}(&slot: ptr) -> i32`
//!   loads each capture out of the slot, generates the closure
//!   body with the captures bound to those loads, and writes the
//!   result back to `slot+8`;
//! - `inline_join` (the capture summary has both an fn-ptr
//!   capture and a non-copy payload capture) emits no helpers:
//!   the body is generated inline at the call site and the slot
//!   is marked with handle `-1`.
//!
//! Capture order is first-use order here; Zig assigns offsets in
//! StringHashMap iteration order. The layout is private to the
//! emitted program (call site and worker are generated from the
//! same table), so the orders need not agree across backends.
//!
//! When any helper is registered, `generate_program` also emits
//! `@import "sa_std/thread.sa"` (the THREAD_SPAWN macro's home),
//! mirroring codegen.zig's conditional import.

use std::collections::HashSet;

use sla_ast::{CallExpr, ClosureLiteral, Node, Type};
use sla_lowering::{is_borrow_like_type, is_internal_symbol, node_key};

use crate::{Result, TextError};

/// lowering_rules.zig `isThreadSpawnCall`: the pure recognition
/// fact for `thread::spawn(closure)` style calls.
pub(crate) fn is_thread_spawn_call(call: &CallExpr) -> bool {
    call.associated_target.as_deref() == Some("thread")
        && call.func_name == "spawn"
        && call.args.len() == 1
}

/// codegen.zig `threadSpawnClosureLiteral`: a closure literal,
/// possibly under a `^` move prefix.
pub(crate) fn thread_spawn_closure_literal(expr: &Node) -> Option<&ClosureLiteral> {
    match expr {
        Node::ClosureLiteral(lit) => Some(lit),
        Node::MoveExpr(mv) => thread_spawn_closure_literal(&mv.expr),
        _ => None,
    }
}

/// lowering_rules.zig `joinHandleInnerType` (via
/// `userDefinedGenericInner`): `JoinHandle<T> -> T`.
fn join_handle_inner_type(ty: &Type) -> Option<Type> {
    match sla_lowering::peel_borrow_pointer_type(ty) {
        Type::UserDefined(ud) if ud.name == "JoinHandle" && ud.generics.len() == 1 => {
            Some(ud.generics[0].clone())
        }
        _ => None,
    }
}

/// codegen.zig `ThreadCapture`.
#[derive(Clone)]
pub(crate) struct ThreadCapture {
    pub name: String,
    pub offset: usize,
    pub is_fn_ptr: bool,
    pub is_noncopy_payload: bool,
    pub ty: Option<Type>,
}

/// codegen.zig `ThreadSpawnHelper`.
#[derive(Clone)]
pub(crate) struct ThreadSpawnHelper {
    pub worker_name: String,
    pub spawn_name: String,
    pub vtable_name: String,
    pub closure: ClosureLiteral,
    pub ret_ty: Type,
    pub captures: Vec<ThreadCapture>,
    pub slot_size: usize,
    pub inline_join: bool,
}

/// codegen.zig `ThreadCaptureInfo` (collection-time facts).
#[derive(Default, Clone)]
struct CaptureInfo {
    is_fn_ptr: bool,
    is_noncopy_payload: bool,
    ty: Option<Type>,
}

impl<'c> crate::Codegen<'c> {
    // -- collection pre-pass (codegen.zig `collectThreadSpawn*`) --

    /// codegen.zig:9368 `collectThreadSpawnHelpers`, driven from
    /// `generate_program` before any emission: walk every
    /// function / method / test body in declaration order and
    /// register a helper per well-formed `thread::spawn` site.
    pub(crate) fn collect_thread_spawn_helpers(&mut self, program: &Node) -> Result<()> {
        let Node::Program(p) = program else {
            return Ok(());
        };
        for decl in &p.decls {
            match decl {
                Node::FuncDecl(f) => {
                    let mut remembered: Vec<(String, Type)> = Vec::new();
                    for param in &f.params {
                        remembered.push((param.name.clone(), (*param.ty).clone()));
                    }
                    self.collect_spawn_in_block(&f.body, &mut remembered);
                }
                Node::ImplDecl(i) => {
                    for method in &i.methods {
                        if let Node::FuncDecl(m) = method {
                            let mut remembered: Vec<(String, Type)> = Vec::new();
                            for param in &m.params {
                                remembered.push((param.name.clone(), (*param.ty).clone()));
                            }
                            self.collect_spawn_in_block(&m.body, &mut remembered);
                        }
                    }
                }
                Node::TestDecl(t) => {
                    let mut remembered: Vec<(String, Type)> = Vec::new();
                    self.collect_spawn_in_block(&t.body, &mut remembered);
                }
                Node::ConstStmt(c) => {
                    let mut remembered: Vec<(String, Type)> = Vec::new();
                    self.collect_spawn_in_expr(&c.value, &mut remembered)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// codegen.zig `collectThreadSpawnInBlock`: in-order walk;
    /// let/const value types are remembered so a later spawn in
    /// the same walk sees let-bound callee types.
    fn collect_spawn_in_block(
        &mut self,
        block: &[Node],
        remembered: &mut Vec<(String, Type)>,
    ) -> Result<()> {
        for stmt in block {
            match stmt {
                Node::LetStmt(l) => {
                    self.collect_spawn_in_expr(&l.value, remembered)?;
                    if let Some(ty) = self.expr_type(&l.value) {
                        remember_type(remembered, &l.name, &ty);
                    }
                }
                Node::LetDestructureStmt(l) => {
                    self.collect_spawn_in_expr(&l.value, remembered)?;
                }
                Node::ConstStmt(c) => {
                    self.collect_spawn_in_expr(&c.value, remembered)?;
                    if let Some(ty) = self.expr_type(&c.value) {
                        remember_type(remembered, &c.name, &ty);
                    }
                }
                Node::AssignStmt(a) => {
                    self.collect_spawn_in_expr(&a.target, remembered)?;
                    self.collect_spawn_in_expr(&a.value, remembered)?;
                }
                Node::BlockStmt(b) => self.collect_spawn_in_block(&b.body, remembered)?,
                Node::ArenaStmt(b) => self.collect_spawn_in_block(&b.body, remembered)?,
                Node::ExprStmt(e) => self.collect_spawn_in_expr(e, remembered)?,
                Node::ReturnStmt(r) => {
                    if let Some(v) = &r.value {
                        self.collect_spawn_in_expr(v, remembered)?;
                    }
                }
                Node::ForStmt(f) => {
                    self.collect_spawn_in_expr(&f.start, remembered)?;
                    if let Some(end) = &f.end {
                        self.collect_spawn_in_expr(end, remembered)?;
                    }
                    self.collect_spawn_in_block(&f.body, remembered)?;
                }
                Node::WhileStmt(w) => {
                    self.collect_spawn_in_expr(&w.cond, remembered)?;
                    self.collect_spawn_in_block(&w.body, remembered)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// codegen.zig `collectThreadSpawnInExpr`.
    fn collect_spawn_in_expr(
        &mut self,
        expr: &Node,
        remembered: &mut Vec<(String, Type)>,
    ) -> Result<()> {
        match expr {
            Node::CallExpr(call) => {
                if is_thread_spawn_call(call) {
                    self.thread_spawn_helper_for_expr(expr, remembered)?;
                }
                for arg in &call.args {
                    self.collect_spawn_in_expr(arg, remembered)?;
                }
            }
            Node::BinaryExpr(bin) => {
                self.collect_spawn_in_expr(&bin.left, remembered)?;
                self.collect_spawn_in_expr(&bin.right, remembered)?;
            }
            Node::BorrowExpr(b) => self.collect_spawn_in_expr(&b.expr, remembered)?,
            Node::MoveExpr(m) => self.collect_spawn_in_expr(&m.expr, remembered)?,
            Node::DerefExpr(d) => self.collect_spawn_in_expr(&d.expr, remembered)?,
            Node::CastExpr(c) => self.collect_spawn_in_expr(&c.expr, remembered)?,
            Node::FieldExpr(f) => self.collect_spawn_in_expr(&f.expr, remembered)?,
            Node::StructLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_spawn_in_expr(&field.value, remembered)?;
                }
            }
            Node::EnumLiteral(lit) => {
                for field in &lit.fields {
                    self.collect_spawn_in_expr(&field.value, remembered)?;
                }
            }
            Node::TupleLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_spawn_in_expr(elem, remembered)?;
                }
            }
            Node::ArrayLiteral(lit) => {
                for elem in &lit.elements {
                    self.collect_spawn_in_expr(elem, remembered)?;
                }
            }
            Node::RepeatArrayLiteral(lit) => {
                self.collect_spawn_in_expr(&lit.value, remembered)?;
            }
            Node::IndexExpr(idx) => {
                self.collect_spawn_in_expr(&idx.target, remembered)?;
                self.collect_spawn_in_expr(&idx.index, remembered)?;
            }
            Node::SliceExpr(slc) => {
                self.collect_spawn_in_expr(&slc.target, remembered)?;
                self.collect_spawn_in_expr(&slc.start, remembered)?;
                self.collect_spawn_in_expr(&slc.end, remembered)?;
            }
            Node::ClosureLiteral(lit) => {
                self.collect_spawn_in_expr(&lit.body, remembered)?;
            }
            Node::AwaitExpr(aw) => self.collect_spawn_in_expr(&aw.expr, remembered)?,
            Node::TryExpr(t) => self.collect_spawn_in_expr(&t.expr, remembered)?,
            Node::IfExpr(ife) => {
                self.collect_spawn_in_expr(&ife.cond, remembered)?;
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        self.collect_spawn_in_expr(&cond.value, remembered)?;
                    }
                }
                self.collect_spawn_in_block(&ife.then_block, remembered)?;
                if let Some(else_block) = &ife.else_block {
                    self.collect_spawn_in_block(else_block, remembered)?;
                }
            }
            Node::SwitchExpr(swe) => {
                self.collect_spawn_in_expr(&swe.val, remembered)?;
                for case in &swe.cases {
                    self.collect_spawn_in_expr(&case.pattern, remembered)?;
                    self.collect_spawn_in_block(&case.body, remembered)?;
                }
            }
            Node::MatchExpr(mat) => {
                self.collect_spawn_in_expr(&mat.val, remembered)?;
                for case in &mat.cases {
                    if let Some(guard) = &case.guard {
                        self.collect_spawn_in_expr(guard, remembered)?;
                    }
                    self.collect_spawn_in_block(&case.body, remembered)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// codegen.zig `threadSpawnHelperForExpr`: register the
    /// helper for one spawn site. Sites whose argument is not a
    /// nullary closure literal register nothing — the call-site
    /// arm then fails loudly, exactly as the Zig lookup miss
    /// fails generation. A missing / non-JoinHandle result type
    /// is a hard error here, as in Zig.
    fn thread_spawn_helper_for_expr(
        &mut self,
        expr: &Node,
        remembered: &[(String, Type)],
    ) -> Result<()> {
        let Node::CallExpr(call) = expr else {
            return Ok(());
        };
        let Some(closure) = thread_spawn_closure_literal(&call.args[0]) else {
            return Ok(());
        };
        if !closure.params.is_empty() {
            return Ok(());
        }
        let key = node_key(expr);
        if self.thread_spawn_helpers.iter().any(|(k, _)| *k == key) {
            return Ok(());
        }
        let join_ty = self.expr_type(expr).ok_or(TextError::MissingType {
            context: "thread::spawn result type".to_string(),
        })?;
        let ret_ty = join_handle_inner_type(&join_ty).ok_or(TextError::MissingType {
            context: "thread::spawn result is not a JoinHandle".to_string(),
        })?;
        let idx = self.thread_helper_idx;
        self.thread_helper_idx += 1;
        let captures = self.collect_thread_closure_captures(closure, remembered);
        let mut slot_size = 16usize;
        for capture in &captures {
            slot_size = capture.offset + 8;
        }
        // lowering_rules `planEscapedClosureExecution`.
        let has_fn_ptr = captures.iter().any(|c| c.is_fn_ptr);
        let has_noncopy = captures.iter().any(|c| c.is_noncopy_payload);
        let helper = ThreadSpawnHelper {
            worker_name: format!("sla_thread_worker_{idx}"),
            spawn_name: format!("sla_thread_spawn_{idx}"),
            vtable_name: format!("SLA_THREAD_VT_{idx}"),
            closure: closure.clone(),
            ret_ty,
            captures,
            slot_size,
            inline_join: has_fn_ptr && has_noncopy,
        };
        self.thread_spawn_helpers.push((key, helper));
        Ok(())
    }

    // -- capture collection (codegen.zig `collectThreadClosure*`) --

    /// codegen.zig `collectThreadClosureCaptures`: walk the
    /// closure body for free identifiers; offsets run 16, 24, ...
    /// in first-capture order.
    fn collect_thread_closure_captures(
        &self,
        closure: &ClosureLiteral,
        remembered: &[(String, Type)],
    ) -> Vec<ThreadCapture> {
        let mut captures: Vec<(String, CaptureInfo)> = Vec::new();
        let mut locals: HashSet<String> = HashSet::new();
        for param in &closure.params {
            locals.insert(param.name.clone());
        }
        self.capture_walk_expr(&closure.body, &mut captures, &mut locals, remembered);
        let mut out = Vec::new();
        let mut offset = 16usize;
        for (name, info) in captures {
            out.push(ThreadCapture {
                name,
                offset,
                is_fn_ptr: info.is_fn_ptr,
                is_noncopy_payload: info.is_noncopy_payload,
                ty: info.ty,
            });
            offset += 8;
        }
        out
    }

    /// codegen.zig `callNameResolvesToValue`: a bare call name
    /// denotes a value in scope — a free function, a macro, an
    /// internal symbol, a global const, or a typed local binding.
    /// Receiver method names denote no value and must never be
    /// recorded as fn-ptr captures (doing so poisons the capture
    /// summary and can flip the plan to inline_join).
    fn call_name_resolves_to_value(&self, name: &str, remembered: &[(String, Type)]) -> bool {
        if self.check.func(name).is_some() {
            return true;
        }
        if self.func_index.iter().any(|(n, _)| n == name) {
            return true;
        }
        if self.check.is_macro(name) {
            return true;
        }
        if is_internal_symbol(name) {
            return true;
        }
        if self.global_const_bindings.contains(name) {
            return true;
        }
        remembered.iter().any(|(n, _)| n == name)
    }

    /// codegen.zig `captureNameFromIdentifier`.
    fn capture_name(
        &self,
        name: &str,
        ty: Option<Type>,
        used_as_fn_ptr: bool,
        captures: &mut Vec<(String, CaptureInfo)>,
        locals: &HashSet<String>,
        remembered: &[(String, Type)],
    ) {
        if locals.contains(name) {
            return;
        }
        if self.global_const_bindings.contains(name) {
            return;
        }
        if self.check.func(name).is_some() || self.func_index.iter().any(|(n, _)| n == name) {
            return;
        }
        if self.check.is_macro(name) {
            return;
        }
        if is_internal_symbol(name) {
            return;
        }
        let pos = match captures.iter().position(|(n, _)| n == name) {
            Some(p) => p,
            None => {
                captures.push((name.to_string(), CaptureInfo::default()));
                captures.len() - 1
            }
        };
        let info = &mut captures[pos].1;
        if used_as_fn_ptr {
            info.is_fn_ptr = true;
        }
        let capture_ty = ty.or_else(|| {
            remembered
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, t)| t.clone())
        });
        if let Some(cty) = capture_ty {
            if info.ty.is_none() {
                info.ty = Some(cty.clone());
            }
            if matches!(cty, Type::FnPtr(_)) {
                info.is_fn_ptr = true;
            }
            if !self.type_is_copy_value(&cty) && !is_borrow_like_type(&cty) {
                info.is_noncopy_payload = true;
            }
        }
    }

    /// codegen.zig `collectThreadClosureCapturesInBlock`.
    fn capture_walk_block(
        &self,
        block: &[Node],
        captures: &mut Vec<(String, CaptureInfo)>,
        locals: &mut HashSet<String>,
        remembered: &[(String, Type)],
    ) {
        for stmt in block {
            match stmt {
                Node::LetStmt(l) => {
                    self.capture_walk_expr(&l.value, captures, locals, remembered);
                    locals.insert(l.name.clone());
                }
                Node::LetElseStmt(l) => {
                    self.capture_walk_expr(&l.value, captures, locals, remembered);
                    for binding in &l.pattern.bindings {
                        locals.insert(binding.clone());
                    }
                    self.capture_walk_block(&l.else_block, captures, locals, remembered);
                }
                Node::LetDestructureStmt(l) => {
                    self.capture_walk_expr(&l.value, captures, locals, remembered);
                    for name in &l.names {
                        locals.insert(name.clone());
                    }
                }
                Node::ConstStmt(c) => {
                    self.capture_walk_expr(&c.value, captures, locals, remembered);
                    locals.insert(c.name.clone());
                }
                Node::AssignStmt(a) => {
                    self.capture_walk_expr(&a.target, captures, locals, remembered);
                    self.capture_walk_expr(&a.value, captures, locals, remembered);
                }
                Node::BlockStmt(b) => {
                    let mut child = locals.clone();
                    self.capture_walk_block(&b.body, captures, &mut child, remembered);
                }
                Node::ArenaStmt(b) => {
                    let mut child = locals.clone();
                    self.capture_walk_block(&b.body, captures, &mut child, remembered);
                }
                Node::ExprStmt(e) => {
                    self.capture_walk_expr(e, captures, locals, remembered)
                }
                Node::ReturnStmt(r) => {
                    if let Some(v) = &r.value {
                        self.capture_walk_expr(v, captures, locals, remembered);
                    }
                }
                Node::ForStmt(f) => {
                    self.capture_walk_expr(&f.start, captures, locals, remembered);
                    if let Some(end) = &f.end {
                        self.capture_walk_expr(end, captures, locals, remembered);
                    }
                    let mut child = locals.clone();
                    child.insert(f.var_name.clone());
                    self.capture_walk_block(&f.body, captures, &mut child, remembered);
                }
                Node::WhileStmt(w) => {
                    self.capture_walk_expr(&w.cond, captures, locals, remembered);
                    let mut child = locals.clone();
                    if let Some(pattern) = &w.let_pattern {
                        for binding in &pattern.bindings {
                            child.insert(binding.clone());
                        }
                    }
                    self.capture_walk_block(&w.body, captures, &mut child, remembered);
                }
                _ => {}
            }
        }
    }

    /// codegen.zig `collectThreadClosureCapturesInExpr`.
    fn capture_walk_expr(
        &self,
        expr: &Node,
        captures: &mut Vec<(String, CaptureInfo)>,
        locals: &mut HashSet<String>,
        remembered: &[(String, Type)],
    ) {
        match expr {
            Node::Identifier(name) => {
                let ty = self.expr_type(expr);
                self.capture_name(name, ty, false, captures, locals, remembered);
            }
            Node::BinaryExpr(bin) => {
                self.capture_walk_expr(&bin.left, captures, locals, remembered);
                self.capture_walk_expr(&bin.right, captures, locals, remembered);
            }
            Node::BorrowExpr(b) => self.capture_walk_expr(&b.expr, captures, locals, remembered),
            Node::MoveExpr(m) => self.capture_walk_expr(&m.expr, captures, locals, remembered),
            Node::DerefExpr(d) => self.capture_walk_expr(&d.expr, captures, locals, remembered),
            Node::CastExpr(c) => self.capture_walk_expr(&c.expr, captures, locals, remembered),
            Node::FieldExpr(f) => self.capture_walk_expr(&f.expr, captures, locals, remembered),
            Node::CallExpr(call) => {
                if call.associated_target.is_none()
                    && call.callee.is_none()
                    && self.call_name_resolves_to_value(&call.func_name, remembered)
                {
                    self.capture_name(&call.func_name, None, true, captures, locals, remembered);
                }
                for arg in &call.args {
                    self.capture_walk_expr(arg, captures, locals, remembered);
                }
            }
            Node::StructLiteral(lit) => {
                for field in &lit.fields {
                    self.capture_walk_expr(&field.value, captures, locals, remembered);
                }
            }
            Node::EnumLiteral(lit) => {
                for field in &lit.fields {
                    self.capture_walk_expr(&field.value, captures, locals, remembered);
                }
            }
            Node::TupleLiteral(lit) => {
                for elem in &lit.elements {
                    self.capture_walk_expr(elem, captures, locals, remembered);
                }
            }
            Node::ArrayLiteral(lit) => {
                for elem in &lit.elements {
                    self.capture_walk_expr(elem, captures, locals, remembered);
                }
            }
            Node::RepeatArrayLiteral(lit) => {
                self.capture_walk_expr(&lit.value, captures, locals, remembered);
            }
            Node::IndexExpr(idx) => {
                self.capture_walk_expr(&idx.target, captures, locals, remembered);
                self.capture_walk_expr(&idx.index, captures, locals, remembered);
            }
            Node::SliceExpr(slc) => {
                self.capture_walk_expr(&slc.target, captures, locals, remembered);
                self.capture_walk_expr(&slc.start, captures, locals, remembered);
                self.capture_walk_expr(&slc.end, captures, locals, remembered);
            }
            // A nested closure literal is not descended into.
            Node::ClosureLiteral(_) => {}
            Node::AwaitExpr(aw) => self.capture_walk_expr(&aw.expr, captures, locals, remembered),
            Node::TryExpr(t) => self.capture_walk_expr(&t.expr, captures, locals, remembered),
            Node::IfExpr(ife) => {
                self.capture_walk_expr(&ife.cond, captures, locals, remembered);
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        self.capture_walk_expr(&cond.value, captures, locals, remembered);
                    }
                }
                let mut then_locals = locals.clone();
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        for binding in &cond.pattern.bindings {
                            then_locals.insert(binding.clone());
                        }
                    }
                }
                self.capture_walk_block(&ife.then_block, captures, &mut then_locals, remembered);
                if let Some(else_block) = &ife.else_block {
                    let mut else_locals = locals.clone();
                    self.capture_walk_block(else_block, captures, &mut else_locals, remembered);
                }
            }
            Node::SwitchExpr(swe) => {
                self.capture_walk_expr(&swe.val, captures, locals, remembered);
                for case in &swe.cases {
                    let mut child = locals.clone();
                    self.capture_walk_block(&case.body, captures, &mut child, remembered);
                }
            }
            Node::MatchExpr(mat) => {
                self.capture_walk_expr(&mat.val, captures, locals, remembered);
                for case in &mat.cases {
                    if let Some(guard) = &case.guard {
                        self.capture_walk_expr(guard, captures, locals, remembered);
                    }
                    let mut child = locals.clone();
                    for binding in &case.pattern.bindings {
                        child.insert(binding.clone());
                    }
                    self.capture_walk_block(&case.body, captures, &mut child, remembered);
                }
            }
            Node::UnsafeExpr(ue) => {
                let mut child = locals.clone();
                self.capture_walk_block(&ue.body, captures, &mut child, remembered);
            }
            _ => {}
        }
    }

}

/// Zig `rememberLocalBindingType` over the collector's scratch
/// table: last write wins, position kept.
fn remember_type(remembered: &mut Vec<(String, Type)>, name: &str, ty: &Type) {
    for (n, t) in remembered.iter_mut() {
        if n == name {
            *t = ty.clone();
            return;
        }
    }
    remembered.push((name.to_string(), ty.clone()));
}
