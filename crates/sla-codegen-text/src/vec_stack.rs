//! Task 137 (P1): stack-allocated Vec header — escape analysis.
//!
//! The churn residual (tasks 130/135): a `Vec::new()` binding costs one
//! heap `calloc(1,24)` for its 24-byte header plus two frees per round
//! (7.2 ms per 200k rounds, the whole fixed-cost gap), while rustc
//! keeps the header in the caller's frame. This analysis proves, for
//! one `let v: Vec<T> = Vec::new()` (or `vec(...)`) binding, that the
//! header address never outlives the binding's own scope, so the
//! compiler may emit the header on the stack (`stack_alloc Vec_SIZE`
//! + `VEC_NEW_STACK`, buffer-only free at scope exit) instead of the
//! heap (`VEC_NEW` / `VEC_FREE`).
//!
//! The analysis runs at the `let` statement, over the REMAINING
//! statements of the enclosing block (the binding's whole lifetime:
//! creation to scope exit), and is conservative — any doubt voids the
//! stack header for that binding (it stays heap, exactly as today).
//! It reuses the task-114 lift's walker shape (`vec_lift.rs`) but
//! with P1 rules (DESIGN.md §5a):
//!
//! - ALLOW (the header address is only ever read through the pointer,
//!   never retained, and only in the SAME function — inside a closure
//!   body, a `go`/`defer` call or a loop condition ANY use of the
//!   binding voids): scalar push/pop on a plain identifier receiver;
//!   index/slice reads; `len` / `capacity` / `is_empty` reads;
//!   `clear` / `truncate` (field stores through the pointer);
//!   the reserve family (`reserve` / `extend` / `append`, which take
//!   the header by pointer and do not retain it). NOTE on the current
//!   surface: `capacity` / `is_empty` / `truncate` / `reserve` /
//!   `extend` / `append` / `clear` are NOT surfaced for Vec in this
//!   SLA (0 mentions in surface.rs; corpus 121_vec_rebind: "SLA 无
//!   `Vec.clear()`"), so those allow-list entries are vacuous in v1
//!   — listed anyway so a future surface addition is already covered.
//! - DISQUALIFY (any one voids, conservative): `return` of the
//!   binding; storing it into a struct field / array element / tuple
//!   / enum payload / any aggregate literal; passing it as an argument
//!   to any call other than the known Vec method arms above; `&v` /
//!   `^v` (borrow/move of the binding itself — note: the compiler
//!   emits the borrow-form push (`VEC_PUSH_STACK_B`, task 144 shape
//!   (b)) and the borrow-form free (`VEC_FREE_BUF`) for stack bindings,
//!   so no macro moves the register; the `^` in `VEC_PUSH`'s /
//!   `VEC_FREE`'s expansion applies only to heap bindings);
//!   explicit `!v` release; closure capture; `go` spawn with the
//!   binding; assignment `w = v` (move-out); shadowing `let` of the
//!   same name; `v` used as a loop condition; any use the walker does
//!   not recognize (fail safe, as the lift does).
//!
//! Differences from the lift's scan, stated explicitly: the lift
//! disqualifies index reads and `len` calls (it needs the loop body
//! to touch the header ONLY through the lifted locals); P1 needs only
//! that the header ADDRESS never outlives the frame, so reads through
//! the pointer are fine. The lift's scope is one loop body; P1's scope
//! is the binding's whole lifetime (creation to scope exit). A bare
//! `return` elsewhere does NOT disqualify for P1 (unlike the lift,
//! where it bypasses the loop-exit write-back): a return still drops
//! the binding through the scope-exit release, which frees the buffer
//! via VEC_FREE_BUF. Only a return OF the binding disqualifies.
//!
//! The drop side lives in `emit_release` (release.rs): a binding in
//! `stack_header_bindings` takes the `VEC_FREE_BUF` branch instead
//! of `VEC_FREE`. The nested drop paths (drop.rs) need no branch:
//! a stack Vec cannot sit in a struct field / array element / enum
//! payload in v1, because those are escapes (DESIGN.md §4.4).

use sla_ast::{CallExpr, Node};
use sla_lowering::root_identifier;

use crate::codegen::Codegen;
use crate::surface::scalar_vec_inline_eligible;

/// Scan context: how a use of a Vec binding inside this subtree relates
/// to the binding's lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackCtx {
    /// Inside the binding's lifetime, same function: an allowed use
    /// (scalar push/pop, index/slice read, len-family read, ...) is
    /// fine; any other use disqualifies.
    SameFn,
    /// Inside a closure body, a `go`/`defer` call, or a loop condition:
    /// the code runs in a different function (closure / spawned task)
    /// or reads the header at the loop head, so ANY use of the binding
    /// disqualifies.
    Foreign,
}

/// Mutable state of one `plan_vec_stack_header` analysis. Single
/// binding (the `let` being decided), so a plain bool suffices —
/// unlike the lift, which tracks candidates per name.
#[derive(Default)]
struct StackScan {
    /// Any disqualifying use found (or a shadowing `let` of the same
    /// name): the binding stays heap.
    disqualified: bool,
    /// An AST shape the walk does not know: fail safe, no stack header.
    has_unknown: bool,
}

impl<'c> Codegen<'c> {
    /// Analyse one `let name: Vec<T> = Vec::new()` (or `vec(...)`)
    /// binding over `rest` — the remaining statements of its enclosing
    /// block, i.e. the binding's whole lifetime. Returns true when the
    /// header may be stack-allocated. Called from `gen_let_stmt`
    /// before the value is emitted; the emission-time filters (temp
    /// name, assigned-value slot, addressable, captures) are applied
    /// by the caller, mirroring the lift's final filter.
    pub(crate) fn plan_vec_stack_header(&mut self, name: &str, rest: &[Node]) -> bool {
        let mut scan = StackScan::default();
        self.stack_scan_block(rest, name, &mut scan, StackCtx::SameFn);
        !scan.disqualified && !scan.has_unknown
    }

    fn stack_scan_block(
        &mut self,
        block: &[Node],
        name: &str,
        scan: &mut StackScan,
        ctx: StackCtx,
    ) {
        for stmt in block {
            self.stack_scan_stmt(stmt, name, scan, ctx);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn stack_scan_stmt(
        &mut self,
        stmt: &Node,
        name: &str,
        scan: &mut StackScan,
        ctx: StackCtx,
    ) {
        match stmt {
            Node::WhileStmt(w) => {
                // A use of the binding in the loop condition reads the
                // header at the loop head (DESIGN.md §5a: disqualify);
                // the body is the same function, so it scans as SameFn.
                if let Some(pattern) = &w.let_pattern {
                    for binding in &pattern.bindings {
                        if binding == name {
                            scan.disqualified = true;
                        }
                    }
                }
                self.stack_scan_expr(&w.cond, name, scan, StackCtx::Foreign);
                self.stack_scan_block(&w.body, name, scan, StackCtx::SameFn);
            }
            Node::ForStmt(f) => {
                if f.var_name == name {
                    scan.disqualified = true;
                }
                self.stack_scan_expr(&f.start, name, scan, StackCtx::Foreign);
                if let Some(end) = &f.end {
                    self.stack_scan_expr(end, name, scan, StackCtx::Foreign);
                }
                self.stack_scan_block(&f.body, name, scan, StackCtx::SameFn);
            }
            Node::BlockStmt(blk) | Node::ArenaStmt(blk) => {
                self.stack_scan_block(&blk.body, name, scan, ctx);
            }
            Node::LetStmt(l) => {
                // An inner `let` shadows the outer binding for the
                // rest of the block; per DESIGN.md §5a that voids.
                if l.name == name {
                    scan.disqualified = true;
                }
                self.stack_scan_expr(&l.value, name, scan, ctx);
            }
            Node::ConstStmt(c) => {
                if c.name == name {
                    scan.disqualified = true;
                }
                self.stack_scan_expr(&c.value, name, scan, ctx);
            }
            Node::VarStmt(v) => {
                if v.name == name {
                    scan.disqualified = true;
                }
            }
            Node::LetDestructureStmt(l) => {
                if l.names.iter().any(|n| n == name)
                    || l.rest_name.as_deref() == Some(name)
                    || l.rest_alias.as_deref() == Some(name)
                {
                    scan.disqualified = true;
                }
                self.stack_scan_expr(&l.value, name, scan, ctx);
            }
            Node::LetElseStmt(l) => {
                for binding in &l.pattern.bindings {
                    if binding == name {
                        scan.disqualified = true;
                    }
                }
                self.stack_scan_expr(&l.value, name, scan, ctx);
                self.stack_scan_block(&l.else_block, name, scan, ctx);
            }
            Node::AssignStmt(a) => {
                // Reassignment of the binding itself voids (move-out /
                // slot reuse). A store THROUGH the binding (`v[i] = x`)
                // is a buffer write through the header pointer: it
                // never retains the header address, so it is allowed
                // in SameFn — only the non-root parts are scanned.
                // In Foreign context (closure body, `go` call) the
                // store itself uses the binding, so it voids.
                if let Node::Identifier(target) = &*a.target {
                    if target == name {
                        scan.disqualified = true;
                    }
                } else if root_identifier(&a.target) == Some(name) {
                    if ctx == StackCtx::Foreign {
                        scan.disqualified = true;
                    } else {
                        self.stack_scan_assign_place(&a.target, name, scan, ctx);
                    }
                } else {
                    self.stack_scan_expr(&a.target, name, scan, ctx);
                }
                self.stack_scan_expr(&a.value, name, scan, ctx);
            }
            Node::ReturnStmt(r) => {
                // A bare return elsewhere does NOT void (the binding
                // is still dropped through the scope-exit release);
                // only a return OF the binding does, via the value
                // scan below.
                if let Some(value) = &r.value {
                    self.stack_scan_expr(value, name, scan, ctx);
                }
            }
            Node::ReleaseStmt(r) => {
                if r.var_name == name {
                    scan.disqualified = true;
                }
            }
            Node::ExprStmt(expr) => {
                self.stack_scan_expr(expr, name, scan, ctx);
            }
            Node::GoStmt(g) => {
                // The spawned call runs in a different function.
                self.stack_scan_expr(&g.call, name, scan, StackCtx::Foreign);
            }
            Node::DeferStmt(d) => {
                self.stack_scan_expr(&d.call, name, scan, StackCtx::Foreign);
            }
            Node::SelectStmt(s) => {
                for arm in &s.arms {
                    if let Some((binding, closed)) = &arm.pattern {
                        if binding == name || closed.as_deref() == Some(name) {
                            scan.disqualified = true;
                        }
                    }
                    if let Some(recv) = &arm.recv {
                        self.stack_scan_expr(recv, name, scan, ctx);
                    }
                    self.stack_scan_block(&arm.body, name, scan, ctx);
                }
                if let Some(timeout) = &s.timeout {
                    self.stack_scan_expr(&timeout.expr, name, scan, ctx);
                    self.stack_scan_block(&timeout.body, name, scan, ctx);
                }
            }
            Node::BreakStmt(_) | Node::ContinueStmt(_) => {}
            // Declarations never appear as block statements; any
            // other shape is unknown — fail safe.
            _ => {
                scan.has_unknown = true;
            }
        }
    }

    /// Scan an assignment target rooted at the binding (`v[i]`,
    /// `v.f`, `v[i][j]` ...): the root itself is a place write, not
    /// a use — scan only the non-root parts (indices, nested
    /// targets) in the same context.
    fn stack_scan_assign_place(
        &mut self,
        target: &Node,
        name: &str,
        scan: &mut StackScan,
        ctx: StackCtx,
    ) {
        match target {
            Node::IndexExpr(idx) => {
                self.stack_scan_assign_place(&idx.target, name, scan, ctx);
                self.stack_scan_expr(&idx.index, name, scan, ctx);
            }
            Node::SliceExpr(slc) => {
                self.stack_scan_assign_place(&slc.target, name, scan, ctx);
                self.stack_scan_expr(&slc.start, name, scan, ctx);
                self.stack_scan_expr(&slc.end, name, scan, ctx);
            }
            Node::FieldExpr(f) => {
                self.stack_scan_assign_place(&f.expr, name, scan, ctx);
            }
            Node::Identifier(_) => {
                // The root itself: a place write, not a use.
            }
            _ => {
                self.stack_scan_expr(target, name, scan, ctx);
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn stack_scan_expr(
        &mut self,
        expr: &Node,
        name: &str,
        scan: &mut StackScan,
        ctx: StackCtx,
    ) {
        match expr {
            Node::Identifier(id) => {
                // A bare use of the binding (read, call argument,
                // receiver of an unclaimed call, return value, ...).
                if id == name {
                    scan.disqualified = true;
                }
            }
            Node::CallExpr(call) => {
                self.stack_scan_call(call, name, scan, ctx);
            }
            Node::BinaryExpr(bin) => {
                self.stack_scan_expr(&bin.left, name, scan, ctx);
                self.stack_scan_expr(&bin.right, name, scan, ctx);
            }
            Node::IfExpr(ife) => {
                self.stack_scan_expr(&ife.cond, name, scan, ctx);
                if let Some(chain) = &ife.let_chain {
                    for cond in chain {
                        for binding in &cond.pattern.bindings {
                            if binding == name {
                                scan.disqualified = true;
                            }
                        }
                        self.stack_scan_expr(&cond.value, name, scan, ctx);
                    }
                }
                self.stack_scan_block(&ife.then_block, name, scan, ctx);
                if let Some(else_block) = &ife.else_block {
                    self.stack_scan_block(else_block, name, scan, ctx);
                }
            }
            Node::SwitchExpr(swe) => {
                self.stack_scan_expr(&swe.val, name, scan, ctx);
                for case in &swe.cases {
                    self.stack_scan_expr(&case.pattern, name, scan, ctx);
                    self.stack_scan_block(&case.body, name, scan, ctx);
                }
            }
            Node::MatchExpr(mat) => {
                self.stack_scan_expr(&mat.val, name, scan, ctx);
                for case in &mat.cases {
                    for binding in &case.pattern.bindings {
                        if binding == name {
                            scan.disqualified = true;
                        }
                    }
                    if let Some(guard) = &case.guard {
                        self.stack_scan_expr(guard, name, scan, ctx);
                    }
                    self.stack_scan_block(&case.body, name, scan, ctx);
                }
            }
            Node::UnsafeExpr(ue) => {
                self.stack_scan_block(&ue.body, name, scan, ctx);
            }
            Node::AwaitExpr(a) => self.stack_scan_expr(&a.expr, name, scan, ctx),
            Node::TryExpr(t) => self.stack_scan_expr(&t.expr, name, scan, ctx),
            Node::BorrowExpr(b) => self.stack_scan_expr(&b.expr, name, scan, ctx),
            Node::MoveExpr(m) => self.stack_scan_expr(&m.expr, name, scan, ctx),
            Node::DerefExpr(d) => self.stack_scan_expr(&d.expr, name, scan, ctx),
            Node::CastExpr(c) => self.stack_scan_expr(&c.expr, name, scan, ctx),
            Node::FieldExpr(f) => self.stack_scan_expr(&f.expr, name, scan, ctx),
            Node::IndexExpr(idx) => {
                // An index read through the header pointer: allowed
                // in SameFn when the root is the binding (the header
                // address is not retained); the index itself is
                // scanned normally. In Foreign context (closure body,
                // `go` call, loop condition) the read itself uses
                // the binding, so it voids. Any other root scans as
                // a plain expression.
                if root_identifier(&idx.target) == Some(name) {
                    if ctx == StackCtx::Foreign {
                        scan.disqualified = true;
                    }
                    self.stack_scan_expr(&idx.index, name, scan, ctx);
                } else {
                    self.stack_scan_expr(&idx.target, name, scan, ctx);
                    self.stack_scan_expr(&idx.index, name, scan, ctx);
                }
            }
            Node::SliceExpr(slc) => {
                if root_identifier(&slc.target) == Some(name) {
                    if ctx == StackCtx::Foreign {
                        scan.disqualified = true;
                    }
                    self.stack_scan_expr(&slc.start, name, scan, ctx);
                    self.stack_scan_expr(&slc.end, name, scan, ctx);
                } else {
                    self.stack_scan_expr(&slc.target, name, scan, ctx);
                    self.stack_scan_expr(&slc.start, name, scan, ctx);
                    self.stack_scan_expr(&slc.end, name, scan, ctx);
                }
            }
            Node::TupleLiteral(lit) => {
                for elem_node in &lit.elements {
                    self.stack_scan_expr(elem_node, name, scan, ctx);
                }
            }
            Node::ArrayLiteral(lit) => {
                for elem_node in &lit.elements {
                    self.stack_scan_expr(elem_node, name, scan, ctx);
                }
            }
            Node::RepeatArrayLiteral(lit) => {
                self.stack_scan_expr(&lit.value, name, scan, ctx);
            }
            Node::StructLiteral(lit) => {
                for field in &lit.fields {
                    self.stack_scan_expr(&field.value, name, scan, ctx);
                }
                if let Some(update) = &lit.update_expr {
                    self.stack_scan_expr(update, name, scan, ctx);
                }
            }
            Node::EnumLiteral(lit) => {
                for field in &lit.fields {
                    self.stack_scan_expr(&field.value, name, scan, ctx);
                }
            }
            Node::ClosureLiteral(cl) => {
                // The closure body runs in a different function: any
                // use of the binding there disqualifies it.
                for param in &cl.params {
                    if param.name == name {
                        scan.disqualified = true;
                    }
                }
                self.stack_scan_expr(&cl.body, name, scan, StackCtx::Foreign);
            }
            Node::InlineAsmExpr(asm) => {
                for operand in &asm.operands {
                    if operand.var_name == name {
                        scan.disqualified = true;
                    }
                }
            }
            Node::Literal(_) | Node::GenericFuncRef(_) => {}
            _ => {
                scan.has_unknown = true;
            }
        }
    }

    /// One call expression: a claimed Vec method on the binding is
    /// allowed in SameFn (and disqualifying in Foreign); anything
    /// else is scanned argument by argument, so a use of the binding
    /// as a receiver or argument disqualifies it.
    fn stack_scan_call(
        &mut self,
        call: &CallExpr,
        name: &str,
        scan: &mut StackScan,
        ctx: StackCtx,
    ) {
        // Method call on a plain identifier receiver, no callee,
        // no associated target: `v.push(x)`, `v.len()`, `len(v)`.
        if call.callee.is_none()
            && call.associated_target.is_none()
            && !call.args.is_empty()
            && matches!(&call.args[0], Node::Identifier(id) if id == name)
        {
            let claimed = match call.func_name.as_str() {
                // Scalar push/pop: same claim conditions as the lift's
                // inline arms (Vec receiver, scalar-eligible element,
                // no user-fn shadow). A non-scalar push/pop is NOT
                // claimed here (v1 is conservative): it falls through
                // to the argument scan and disqualifies.
                "push" | "pop" => {
                    let arity_ok = (call.func_name == "push" && call.args.len() == 2)
                        || (call.func_name == "pop" && call.args.len() == 1);
                    arity_ok
                        && self.classify_receiver(call).is_some_and(|(tname, generics)| {
                            tname == "Vec"
                                && !generics.is_empty()
                                && scalar_vec_inline_eligible(&generics[0])
                                && !self.user_fn_shadows_surface(call, "Vec", &generics)
                        })
                }
                // Header-field reads and field-store methods: they
                // only touch the header through the pointer.
                // (capacity / is_empty / truncate / reserve / extend /
                // append / clear are not surfaced for Vec in this SLA,
                // so these arms are vacuous in v1 — listed so a future
                // surface addition is already covered.)
                "len" | "capacity" | "is_empty" | "clear" | "truncate" | "reserve"
                | "extend" | "append" => {
                    self.classify_receiver(call).is_some_and(|(tname, generics)| {
                        tname == "Vec"
                            && !self.user_fn_shadows_surface(call, "Vec", &generics)
                    })
                }
                _ => false,
            };
            if claimed {
                match ctx {
                    StackCtx::SameFn => {
                        // The push value argument may itself use the
                        // binding (`v.push(v.len())`): scan it in the
                        // same context.
                        if call.func_name == "push" {
                            self.stack_scan_expr(&call.args[1], name, scan, ctx);
                        }
                        return;
                    }
                    StackCtx::Foreign => {
                        scan.disqualified = true;
                        return;
                    }
                }
            }
        }
        // Not a claimed Vec method on the binding: every argument is
        // a use of whatever binding it names.
        for arg in &call.args {
            self.stack_scan_expr(arg, name, scan, ctx);
        }
        if let Some(callee) = &call.callee {
            self.stack_scan_expr(callee, name, scan, ctx);
        }
    }
}
