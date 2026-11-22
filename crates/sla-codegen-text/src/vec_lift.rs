//! Task 114: Vec-header local lift ("loop-entry scalar promotion").
//!
//! The churn residual (tasks 101/106): inside a hot `while` loop the
//! generated code reloads the Vec's `len`/`cap`/`ptr` header fields from
//! the stack slot on every push/pop iteration and stores `len` back —
//! 5 memory ops per iteration, while rustc keeps the header in
//! registers (2 ops). LLVM cannot promote the header itself because
//! `&v` escapes (the slow-path `VEC_PUSH` expansion and `VEC_FREE`
//! take it, and the runtime call may reallocate), so the header's
//! address-taken slot stays in memory.
//!
//! The lift is LOCAL, not a full SROA: at a qualifying `while` loop's
//! entry, load the Vec's `ptr`/`len`/`cap` into fresh named scalar
//! locals; inside the loop the inline push/pop arms operate only on
//! those locals; at the loop exit, write `len` back to the header and
//! retire the locals. The lift locals' addresses never escape, so
//! LLVM mem2reg promotes them to registers — the same shape as the
//! loop counters (`i`/`j`/`k`), which LLVM already promotes.
//!
//! Soundness (the analysis in `plan_vec_lift` is conservative — any
//! doubt voids the lift for that binding):
//! - Only a plain `while` loop (no `let_pattern`), only bindings
//!   whose ONLY uses in the body are scalar push/pop calls on a plain
//!   identifier receiver (element type satisfies
//!   `scalar_vec_inline_eligible`, same claim conditions as the
//!   inline arms: `Vec` receiver, no user-fn shadow). Any other use
//!   — index/slice read, another method (`len`, `is_empty`, ...),
//!   passing the binding to a call, `&v`/`^v`, a field read, a bare
//!   read, reassignment, an inner `let` shadowing it, an explicit
//!   `!v` release — disqualifies that binding.
//! - The loop condition must not mention the binding (it is evaluated
//!   at the head while the lift is active, so a header read there
//!   would see the stale in-memory value).
//! - No `return` anywhere in the body (any depth, including nested
//!   loops): a return bypasses the loop-exit write-back. `break` is
//!   fine (it jumps to the exit label, where the write-back runs);
//!   `continue` is fine (it jumps to the head; the locals stay live).
//!   SLA has no labeled break/continue, so every break/continue in
//!   the body targets the innermost loop.
//! - Nested loops are not crossed for candidate collection (they get
//!   their own lift decision), but they ARE walked for
//!   disqualification: an index read or other use inside a nested
//!   loop disqualifies for the enclosing lift, while a push/pop
//!   inside a nested loop is compatible (it reuses the enclosing
//!   lift through the lift map). A nested loop's condition is walked
//!   as Foreign (any use disqualifies), for the same reason as the
//!   outer condition. A loop only lifts bindings not already lifted
//!   by an enclosing loop (the lift stack is consulted; the inner
//!   loop reuses the outer lift's locals).
//! - Closures, `go`/`defer` calls: their bodies run in a different
//!   function, so any use of the binding there disqualifies it, and
//!   a `return` inside one does not count as a body return.
//! - The binding must materialize as its own `.sa` name at the
//!   push/pop sites: if `gen_expr` would return a temp for it
//!   (assigned-value-slot binding, or addressable + stack-alloc) or
//!   the resolved name is a temp (aliased), or it is a thread
//!   capture / closure parameter, the lift is voided — otherwise the
//!   lift key would not match the receiver the surface arms see.
//!
//! Emission notes (`.sa` discipline, verified against churn.sa):
//! - `{slot} = {tmp}` is an SA-level MOVE: the source temp is dead
//!   after it and must not be read or `!`'d again (a later read
//!   traps UseAfterMove). So in the lifted arms every use of the
//!   new-length temp precedes the assignment to the lift local, and
//!   the moved-from temp is never released.
//! - Reassigning a named slot retires its old value first (`!slot`,
//!   the `!j` / `j = tmp_35` counter pattern). The first (setup)
//!   assignment has no old value, so no `!`.
//! - A temp used as an instruction operand is retired with `!` after
//!   its last use. The lift locals are retired at the loop exit,
//!   after the write-back store reads the len local.
//! - Push slow path: the runtime `VEC_PUSH` reads the header, so the
//!   lifted len is synced back first (only len can be stale — the
//!   fast path never changes ptr/cap); after the call, all three
//!   fields are reloaded into the lift locals (growth may have
//!   reallocated). Pop has no runtime call on its fast path and its
//!   NONE arm panics, so it needs no sync.

use std::collections::{HashMap, HashSet};

use sla_ast::{CallExpr, Node, Type, WhileStmt};

use crate::codegen::Codegen;
use crate::surface::scalar_vec_inline_eligible;

/// One lifted Vec binding: the receiver's resolved `.sa` name plus the
/// three fresh scalar locals holding its header fields while the loop
/// runs. The locals are named `__vec_lift{n}_ptr/len/cap` from the
/// per-`Codegen` counter `vec_lift_idx`, so they never collide with a
/// user binding or a generated temp.
#[derive(Debug, Clone)]
pub(crate) struct VecHeaderLift {
    /// Resolved `.sa` name of the Vec binding (what `gen_expr` returns
    /// for the receiver identifier).
    pub recv: String,
    /// Local holding the buffer pointer.
    pub ptr_local: String,
    /// Local holding the element count.
    pub len_local: String,
    /// Local holding the capacity.
    pub cap_local: String,
}

/// Scan context: how a use of a Vec binding inside this subtree relates
/// to the enclosing loop's lift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LiftCtx {
    /// Direct body of the loop being analysed: a scalar push/pop
    /// receiver is a lift candidate; any other use disqualifies.
    Direct,
    /// Inside a nested loop: a scalar push/pop on the binding is
    /// compatible with the enclosing lift (the nested loop reuses it
    /// through the lift map), so it neither records nor disqualifies;
    /// any other use disqualifies.
    Nested,
    /// Inside a closure body, a `go`/`defer` call, or a loop
    /// condition: the code runs in a different function (closure /
    /// spawned task) or reads the header while the lift is active
    /// (condition), so ANY use disqualifies; a `return` here does not
    /// return from the enclosing function, so it is not recorded.
    Foreign,
}

/// Mutable state of one `plan_vec_lift` analysis.
#[derive(Default)]
struct LiftScan {
    /// identifier -> element type, for receivers of claimed scalar
    /// push/pop calls in the direct body.
    candidates: HashMap<String, Type>,
    /// Identifiers with any non-push/pop use (index, other method,
    /// borrow, move, reassign, shadow, bare use, release, ...), or
    /// with disagreeing recorded element types.
    disqualified: HashSet<String>,
    /// A `return` anywhere in the body (any depth, including nested
    /// loops, but not inside a closure) bypasses the loop-exit
    /// write-back, so the whole loop is ineligible.
    has_return: bool,
    /// An AST shape the walk does not know: fail safe, no lift.
    has_unknown: bool,
}

impl<'c> Codegen<'c> {
    /// Analyse a plain `while` loop for Vec-header lifts. Returns the
    /// lifts to install (empty when the loop does not qualify). Only
    /// called on the plain-condition path; the while-let path never
    /// lifts.
    pub(crate) fn plan_vec_lift(&mut self, w: &WhileStmt) -> Vec<VecHeaderLift> {
        let mut scan = LiftScan::default();
        // The condition is evaluated at the head on every iteration
        // while the lift is active, so any use of a binding there
        // disqualifies it.
        self.lift_scan_expr(&w.cond, &mut scan, LiftCtx::Foreign);
        self.lift_scan_block(&w.body, &mut scan, LiftCtx::Direct);
        if scan.has_return || scan.has_unknown {
            return Vec::new();
        }
        // Deterministic order: sort candidates by identifier so the
        // setup/write-back emission (and the lift-local numbering)
        // does not depend on hash iteration order.
        let mut names: Vec<(String, Type)> = scan
            .candidates
            .into_iter()
            .filter(|(name, _)| !scan.disqualified.contains(name))
            .collect();
        names.sort_by(|a, b| a.0.cmp(&b.0));
        let mut lifts = Vec::new();
        for (name, _elem) in names {
            let sa_name = self.resolve_binding_name(&name);
            // The surface arms see whatever `gen_expr` returns for the
            // receiver. If that is a temp (aliased binding, or a value
            // that materializes via a load), the lift key would not
            // match — void the lift rather than emit a stale header
            // read.
            if sla_lowering::is_temporary_register_name(&sa_name) {
                continue;
            }
            if self.assigned_value_slots.contains(&sa_name) {
                continue;
            }
            if self.addressable_bindings.contains(&name)
                && self.stack_alloc_bindings.contains(&sa_name)
            {
                continue;
            }
            // Captures and closure parameters do not read as the
            // binding's own name at the push/pop sites.
            if self.thread_capture_regs.contains_key(&name) {
                continue;
            }
            if self
                .closure_param_regs
                .iter()
                .any(|(param, _)| param == &name)
            {
                continue;
            }
            // Already lifted by an enclosing loop: reuse the outer
            // lift's locals, do not create a second one.
            if self
                .vec_header_lifts
                .iter()
                .any(|lift| lift.recv == sa_name)
            {
                continue;
            }
            let idx = self.vec_lift_idx;
            self.vec_lift_idx += 1;
            lifts.push(VecHeaderLift {
                recv: sa_name,
                ptr_local: format!("__vec_lift{idx}_ptr"),
                len_local: format!("__vec_lift{idx}_len"),
                cap_local: format!("__vec_lift{idx}_cap"),
            });
        }
        lifts
    }

    /// Look up the active lift for a receiver `.sa` name, innermost
    /// loop first. Used by the inline push/pop arms.
    pub(crate) fn lifted_vec_header(&self, recv: &str) -> Option<&VecHeaderLift> {
        self.vec_header_lifts
            .iter()
            .rev()
            .find(|lift| lift.recv == recv)
    }

    /// Emit the loop-entry setup loads for `lifts`: each header field
    /// is loaded into its lift local (a move, so the temp needs no
    /// `!`). Runs once, before the loop head.
    pub(crate) fn emit_vec_lift_setup(&mut self, lifts: &[VecHeaderLift]) {
        for lift in lifts {
            let tmp = self.new_tmp();
            self.ins(&format!("{tmp} = load {}+Vec_ptr as ptr", lift.recv));
            self.ins(&format!("{} = {tmp}", lift.ptr_local));
            let tmp = self.new_tmp();
            self.ins(&format!("{tmp} = load {}+Vec_len as u64", lift.recv));
            self.ins(&format!("{} = {tmp}", lift.len_local));
            let tmp = self.new_tmp();
            self.ins(&format!("{tmp} = load {}+Vec_cap as u64", lift.recv));
            self.ins(&format!("{} = {tmp}", lift.cap_local));
        }
    }

    /// Emit the loop-exit write-back for `lifts`: store the lifted len
    /// back to the header, then retire the lift locals (the store
    /// reads the len local as an operand, so its `!` comes after).
    /// Runs at the exit label, so `break` hits it too.
    pub(crate) fn emit_vec_lift_writeback(&mut self, lifts: &[VecHeaderLift]) {
        for lift in lifts {
            self.ins(&format!(
                "store {}+Vec_len, {} as u64",
                lift.recv, lift.len_local
            ));
            self.w(&format!("    !{}\n", lift.len_local));
            self.w(&format!("    !{}\n", lift.ptr_local));
            self.w(&format!("    !{}\n", lift.cap_local));
        }
    }

    fn lift_scan_block(&mut self, block: &[Node], scan: &mut LiftScan, ctx: LiftCtx) {
        for stmt in block {
            self.lift_scan_stmt(stmt, scan, ctx);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn lift_scan_stmt(&mut self, stmt: &Node, scan: &mut LiftScan, ctx: LiftCtx) {
        match stmt {
            Node::WhileStmt(w) => {
                // Nested loop: its condition reads the header while the
                // enclosing lift is active (Foreign); its body reuses
                // the enclosing lift for push/pop (Nested).
                if let Some(pattern) = &w.let_pattern {
                    for binding in &pattern.bindings {
                        scan.disqualified.insert(binding.clone());
                    }
                }
                self.lift_scan_expr(&w.cond, scan, LiftCtx::Foreign);
                self.lift_scan_block(&w.body, scan, LiftCtx::Nested);
            }
            Node::ForStmt(f) => {
                scan.disqualified.insert(f.var_name.clone());
                self.lift_scan_expr(&f.start, scan, LiftCtx::Foreign);
                if let Some(end) = &f.end {
                    self.lift_scan_expr(end, scan, LiftCtx::Foreign);
                }
                self.lift_scan_block(&f.body, scan, LiftCtx::Nested);
            }
            Node::BlockStmt(blk) | Node::ArenaStmt(blk) => {
                self.lift_scan_block(&blk.body, scan, ctx);
            }
            Node::LetStmt(l) => {
                // An inner `let` shadows the outer binding for the
                // rest of the block; the name is no longer the same
                // binding at later uses.
                scan.disqualified.insert(l.name.clone());
                self.lift_scan_expr(&l.value, scan, ctx);
            }
            Node::ConstStmt(c) => {
                scan.disqualified.insert(c.name.clone());
                self.lift_scan_expr(&c.value, scan, ctx);
            }
            Node::VarStmt(v) => {
                scan.disqualified.insert(v.name.clone());
            }
            Node::LetDestructureStmt(l) => {
                for name in &l.names {
                    scan.disqualified.insert(name.clone());
                }
                if let Some(rest) = &l.rest_name {
                    scan.disqualified.insert(rest.clone());
                }
                if let Some(rest) = &l.rest_alias {
                    scan.disqualified.insert(rest.clone());
                }
                self.lift_scan_expr(&l.value, scan, ctx);
            }
            Node::LetElseStmt(l) => {
                for binding in &l.pattern.bindings {
                    scan.disqualified.insert(binding.clone());
                }
                self.lift_scan_expr(&l.value, scan, ctx);
                self.lift_scan_block(&l.else_block, scan, ctx);
            }
            Node::AssignStmt(a) => {
                if let Node::Identifier(name) = &*a.target {
                    scan.disqualified.insert(name.clone());
                }
                self.lift_scan_expr(&a.target, scan, ctx);
                self.lift_scan_expr(&a.value, scan, ctx);
            }
            Node::ReturnStmt(r) => {
                if ctx != LiftCtx::Foreign {
                    scan.has_return = true;
                }
                if let Some(value) = &r.value {
                    self.lift_scan_expr(value, scan, ctx);
                }
            }
            Node::ReleaseStmt(r) => {
                scan.disqualified.insert(r.var_name.clone());
            }
            Node::ExprStmt(expr) => {
                self.lift_scan_expr(expr, scan, ctx);
            }
            Node::GoStmt(g) => {
                // The spawned call runs in a different function.
                self.lift_scan_expr(&g.call, scan, LiftCtx::Foreign);
            }
            Node::DeferStmt(d) => {
                self.lift_scan_expr(&d.call, scan, LiftCtx::Foreign);
            }
            Node::SelectStmt(s) => {
                for arm in &s.arms {
                    if let Some((binding, closed)) = &arm.pattern {
                        scan.disqualified.insert(binding.clone());
                        if let Some(closed) = closed {
                            scan.disqualified.insert(closed.clone());
                        }
                    }
                    if let Some(recv) = &arm.recv {
                        self.lift_scan_expr(recv, scan, ctx);
                    }
                    self.lift_scan_block(&arm.body, scan, ctx);
                }
                if let Some(timeout) = &s.timeout {
                    self.lift_scan_expr(&timeout.expr, scan, ctx);
                    self.lift_scan_block(&timeout.body, scan, ctx);
                }
            }
            Node::BreakStmt(_) | Node::ContinueStmt(_) => {}
            // Declarations never appear as loop-body statements; any
            // other shape is unknown — fail safe.
            _ => {
                scan.has_unknown = true;
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn lift_scan_expr(&mut self, expr: &Node, scan: &mut LiftScan, ctx: LiftCtx) {
        match expr {
            Node::Identifier(name) => {
                // A bare use of the binding (read, call argument,
                // receiver of an unclaimed call, ...).
                scan.disqualified.insert(name.clone());
            }
            Node::CallExpr(call) => {
                self.lift_scan_call(call, scan, ctx);
            }
            Node::BinaryExpr(bin) => {
                self.lift_scan_expr(&bin.left, scan, ctx);
                self.lift_scan_expr(&bin.right, scan, ctx);
            }
            Node::IfExpr(ife) => {
                self.lift_scan_expr(&ife.cond, scan, ctx);
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        for binding in &cond.pattern.bindings {
                            scan.disqualified.insert(binding.clone());
                        }
                        self.lift_scan_expr(&cond.value, scan, ctx);
                    }
                }
                self.lift_scan_block(&ife.then_block, scan, ctx);
                if let Some(else_block) = &ife.else_block {
                    self.lift_scan_block(else_block, scan, ctx);
                }
            }
            Node::SwitchExpr(swe) => {
                self.lift_scan_expr(&swe.val, scan, ctx);
                for case in &swe.cases {
                    self.lift_scan_expr(&case.pattern, scan, ctx);
                    self.lift_scan_block(&case.body, scan, ctx);
                }
            }
            Node::MatchExpr(mat) => {
                self.lift_scan_expr(&mat.val, scan, ctx);
                for case in &mat.cases {
                    for binding in &case.pattern.bindings {
                        scan.disqualified.insert(binding.clone());
                    }
                    if let Some(guard) = &case.guard {
                        self.lift_scan_expr(guard, scan, ctx);
                    }
                    self.lift_scan_block(&case.body, scan, ctx);
                }
            }
            Node::UnsafeExpr(ue) => {
                self.lift_scan_block(&ue.body, scan, ctx);
            }
            Node::AwaitExpr(a) => self.lift_scan_expr(&a.expr, scan, ctx),
            Node::TryExpr(t) => self.lift_scan_expr(&t.expr, scan, ctx),
            Node::BorrowExpr(b) => self.lift_scan_expr(&b.expr, scan, ctx),
            Node::MoveExpr(m) => self.lift_scan_expr(&m.expr, scan, ctx),
            Node::DerefExpr(d) => self.lift_scan_expr(&d.expr, scan, ctx),
            Node::CastExpr(c) => self.lift_scan_expr(&c.expr, scan, ctx),
            Node::FieldExpr(f) => self.lift_scan_expr(&f.expr, scan, ctx),
            Node::IndexExpr(idx) => {
                self.lift_scan_expr(&idx.target, scan, ctx);
                self.lift_scan_expr(&idx.index, scan, ctx);
            }
            Node::SliceExpr(slc) => {
                self.lift_scan_expr(&slc.target, scan, ctx);
                self.lift_scan_expr(&slc.start, scan, ctx);
                self.lift_scan_expr(&slc.end, scan, ctx);
            }
            Node::TupleLiteral(lit) => {
                for elem in &lit.elements {
                    self.lift_scan_expr(elem, scan, ctx);
                }
            }
            Node::ArrayLiteral(lit) => {
                for elem in &lit.elements {
                    self.lift_scan_expr(elem, scan, ctx);
                }
            }
            Node::RepeatArrayLiteral(lit) => {
                self.lift_scan_expr(&lit.value, scan, ctx);
            }
            Node::StructLiteral(lit) => {
                for field in &lit.fields {
                    self.lift_scan_expr(&field.value, scan, ctx);
                }
                if let Some(update) = &lit.update_expr {
                    self.lift_scan_expr(update, scan, ctx);
                }
            }
            Node::EnumLiteral(lit) => {
                for field in &lit.fields {
                    self.lift_scan_expr(&field.value, scan, ctx);
                }
            }
            Node::ClosureLiteral(cl) => {
                // The closure body runs in a different function: any
                // use of the binding there disqualifies it, and a
                // `return` there does not return from the enclosing
                // function (save/restore `has_return` in case a
                // nested loop inside the closure re-enters Nested
                // context).
                for param in &cl.params {
                    scan.disqualified.insert(param.name.clone());
                }
                let saved_return = scan.has_return;
                self.lift_scan_expr(&cl.body, scan, LiftCtx::Foreign);
                scan.has_return = saved_return;
            }
            Node::InlineAsmExpr(asm) => {
                for operand in &asm.operands {
                    scan.disqualified.insert(operand.var_name.clone());
                }
            }
            Node::Literal(_) | Node::GenericFuncRef(_) => {}
            _ => {
                scan.has_unknown = true;
            }
        }
    }

    /// One call expression: a claimed scalar Vec push/pop on a plain
    /// identifier receiver is a candidate (Direct), compatible
    /// (Nested), or disqualifying (Foreign); anything else is scanned
    /// argument by argument, so a use of a binding as a receiver or
    /// argument disqualifies it.
    fn lift_scan_call(&mut self, call: &CallExpr, scan: &mut LiftScan, ctx: LiftCtx) {
        if (call.func_name == "push" || call.func_name == "pop")
            && call.callee.is_none()
            && call.associated_target.is_none()
        {
            let arity_ok = (call.func_name == "push" && call.args.len() == 2)
                || (call.func_name == "pop" && call.args.len() == 1);
            if arity_ok {
                if let Node::Identifier(name) = &call.args[0] {
                    if let Some((tname, generics)) = self.classify_receiver(call) {
                        if tname == "Vec"
                            && !generics.is_empty()
                            && scalar_vec_inline_eligible(&generics[0])
                            && !self.user_fn_shadows_surface(call, "Vec", &generics)
                        {
                            match ctx {
                                LiftCtx::Direct => {
                                    let elem = generics[0].clone();
                                    match scan.candidates.get(name) {
                                        Some(prev) if *prev != elem => {
                                            // Same binding, disagreeing
                                            // recorded element type:
                                            // do not trust either.
                                            scan.disqualified.insert(name.clone());
                                        }
                                        Some(_) => {}
                                        None => {
                                            scan.candidates.insert(name.clone(), elem);
                                        }
                                    }
                                }
                                LiftCtx::Nested => {
                                    // Compatible with the enclosing
                                    // lift; neither records nor
                                    // disqualifies.
                                }
                                LiftCtx::Foreign => {
                                    scan.disqualified.insert(name.clone());
                                }
                            }
                            // The value argument (push) may itself use
                            // the binding (`v.push(v.len())`): scan it
                            // in the same context.
                            if call.func_name == "push" {
                                self.lift_scan_expr(&call.args[1], scan, ctx);
                            }
                            return;
                        }
                    }
                }
            }
        }
        // Not a claimed scalar push/pop: every argument is a use of
        // whatever binding it names.
        for arg in &call.args {
            self.lift_scan_expr(arg, scan, ctx);
        }
        if let Some(callee) = &call.callee {
            self.lift_scan_expr(callee, scan, ctx);
        }
    }
}
