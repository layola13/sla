//! Spawn-boundary borrow guard for `thread::spawn` closures — Rust port
//! of the Zig `checkSpawnClosureBorrowEscape` (sa_plugin_sla
//! `src/type_checker.zig`, landed as 2b65a1dd; defect
//! DEF-SLA-SPAWN-CLOSURE-BORROW-ESCAPE).
//!
//! The `go` boundary has always rejected borrowed spawn payloads
//! (`go_type_has_borrow`, check_go.rs), but `thread::spawn` audited
//! only the closure's arity and type, so a closure borrowing an
//! enclosing-scope place — `thread::spawn(^|| modify(&number))` —
//! checked clean and raced at runtime. This guard walks the closure
//! body after the ordinary closure check and rejects, with
//! `error.BorrowEscape`:
//!   1. a borrow (`&place`) whose root identifier is an
//!      enclosing-scope binding (not bound inside the closure);
//!   2. a use of an enclosing-scope binding whose type contains a
//!      borrow (the indirect form `let r = &number; spawn(^|| f(r))`);
//!   3. an assignment whose target place is rooted at an
//!      enclosing-scope binding.
//!
//! The predicate is borrow-only on purpose: the async thread pool
//! legitimately captures futures and `Arc` clones into spawned
//! closures, and those stay legal (futures/closures are not borrows
//! for this guard, mirroring the Zig `typeContainsBorrow` choice).
//! Stored-closure and plain-fn-value spawns are NOT covered here —
//! the former is check-accepted but fails loudly at codegen, the
//! latter is already rejected as a type mismatch — same residuals
//! as the Zig side.

use sla_ast::node::Node;

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};

fn spawn_escape(msg: String) -> Diagnostic {
    Diagnostic::new(ErrorKind::BorrowEscape, msg)
}

/// Root binding of a place expression, walking field / index / deref /
/// borrow / move wrappers (mirrors Zig `placeRootName`).
fn spawn_place_root(node: &Node) -> Option<&str> {
    match node {
        Node::Identifier(name) => Some(name.as_str()),
        Node::FieldExpr(f) => spawn_place_root(&f.expr),
        Node::IndexExpr(i) => spawn_place_root(&i.target),
        Node::DerefExpr(d) => spawn_place_root(&d.expr),
        Node::BorrowExpr(b) => spawn_place_root(&b.expr),
        Node::MoveExpr(m) => spawn_place_root(&m.expr),
        _ => None,
    }
}

impl Checker {
    /// Audit a `thread::spawn` argument for borrow escape. The argument
    /// is expected to be a closure literal, optionally under the `^`
    /// move prefix; any other shape is left alone (stored closures and
    /// fn values keep their existing disciplines).
    pub fn check_spawn_closure_borrow_escape(&self, arg: &Node) -> CheckResult<()> {
        let mut node = arg;
        if let Node::MoveExpr(m) = node {
            node = &m.expr;
        }
        let Node::ClosureLiteral(closure) = node else {
            return Ok(());
        };
        let mut inner: Vec<String> =
            closure.params.iter().map(|p| p.name.clone()).collect();
        self.spawn_walk_node(&closure.body, &mut inner)
    }

    /// Walk one statement block with block-local scoping: names bound by
    /// `let`/`const`/destructure inside the block are visible to the
    /// rest of the block only.
    fn spawn_walk_block(&self, stmts: &[Node], inner: &mut Vec<String>) -> CheckResult<()> {
        let mark = inner.len();
        let mut result = Ok(());
        for stmt in stmts {
            if let Err(e) = self.spawn_walk_node(stmt, inner) {
                result = Err(e);
                break;
            }
        }
        inner.truncate(mark);
        result
    }

    fn spawn_walk_node(&self, node: &Node, inner: &mut Vec<String>) -> CheckResult<()> {
        match node {
            Node::BorrowExpr(b) => {
                if let Some(root) = spawn_place_root(&b.expr) {
                    if !inner.iter().any(|n| n == root) && self.lookup_ty(root).is_some() {
                        return Err(spawn_escape(format!(
                            "BorrowEscape: thread::spawn: closure borrows `{root}` from the enclosing scope, which would dangle or race across the thread boundary; pass an owned value (such as an Arc clone) or a raw pointer instead"
                        )));
                    }
                }
                self.spawn_walk_node(&b.expr, inner)
            }
            Node::Identifier(name) => {
                if !inner.iter().any(|n| n == name) {
                    if let Some(ty) = self.lookup_ty(name) {
                        if self.go_type_has_borrow(&ty, 0) {
                            return Err(spawn_escape(format!(
                                "BorrowEscape: thread::spawn: closure captures borrowed value `{name}` from the enclosing scope, which would dangle or race across the thread boundary; pass an owned value (such as an Arc clone) or a raw pointer instead"
                            )));
                        }
                    }
                }
                Ok(())
            }
            Node::AssignStmt(a) => {
                if let Some(root) = spawn_place_root(&a.target) {
                    if !inner.iter().any(|n| n == root) && self.lookup_ty(root).is_some() {
                        return Err(spawn_escape(format!(
                            "BorrowEscape: thread::spawn: closure writes `{root}` in the enclosing scope, which would race across the thread boundary; pass an owned value (such as an Arc clone) or a raw pointer instead"
                        )));
                    }
                }
                self.spawn_walk_node(&a.target, inner)?;
                self.spawn_walk_node(&a.value, inner)
            }
            Node::ClosureLiteral(c) => {
                let mark = inner.len();
                for p in &c.params {
                    inner.push(p.name.clone());
                }
                let result = self.spawn_walk_node(&c.body, inner);
                inner.truncate(mark);
                result
            }
            Node::LetStmt(l) => {
                self.spawn_walk_node(&l.value, inner)?;
                inner.push(l.name.clone());
                Ok(())
            }
            Node::ConstStmt(c) => {
                self.spawn_walk_node(&c.value, inner)?;
                inner.push(c.name.clone());
                Ok(())
            }
            Node::VarStmt(v) => {
                inner.push(v.name.clone());
                Ok(())
            }
            Node::LetElseStmt(l) => {
                self.spawn_walk_node(&l.value, inner)?;
                // The else block diverges; pattern bindings do not
                // apply inside it.
                self.spawn_walk_block(&l.else_block, inner)?;
                for b in &l.pattern.bindings {
                    inner.push(b.clone());
                }
                Ok(())
            }
            Node::LetDestructureStmt(l) => {
                self.spawn_walk_node(&l.value, inner)?;
                for name in &l.names {
                    inner.push(name.clone());
                }
                if let Some(rest) = &l.rest_name {
                    inner.push(rest.clone());
                }
                if let Some(alias) = &l.rest_alias {
                    inner.push(alias.clone());
                }
                Ok(())
            }
            Node::IfExpr(i) => {
                self.spawn_walk_node(&i.cond, inner)?;
                if let Some(chain) = &i.let_chain {
                    let mark = inner.len();
                    for cond in chain {
                        self.spawn_walk_node(&cond.value, inner)?;
                        for b in &cond.pattern.bindings {
                            inner.push(b.clone());
                        }
                    }
                    let result = self.spawn_walk_block(&i.then_block, inner);
                    inner.truncate(mark);
                    result?;
                } else {
                    self.spawn_walk_block(&i.then_block, inner)?;
                }
                if let Some(else_block) = &i.else_block {
                    self.spawn_walk_block(else_block, inner)?;
                }
                Ok(())
            }
            Node::MatchExpr(m) => {
                self.spawn_walk_node(&m.val, inner)?;
                for case in &m.cases {
                    let mark = inner.len();
                    for b in &case.pattern.bindings {
                        inner.push(b.clone());
                    }
                    if let Some(guard) = &case.guard {
                        self.spawn_walk_node(guard, inner)?;
                    }
                    let result = self.spawn_walk_block(&case.body, inner);
                    inner.truncate(mark);
                    result?;
                }
                Ok(())
            }
            Node::SwitchExpr(s) => {
                self.spawn_walk_node(&s.val, inner)?;
                for case in &s.cases {
                    self.spawn_walk_node(&case.pattern, inner)?;
                    self.spawn_walk_block(&case.body, inner)?;
                }
                Ok(())
            }
            Node::ForStmt(f) => {
                self.spawn_walk_node(&f.start, inner)?;
                if let Some(end) = &f.end {
                    self.spawn_walk_node(end, inner)?;
                }
                let mark = inner.len();
                inner.push(f.var_name.clone());
                let result = self.spawn_walk_block(&f.body, inner);
                inner.truncate(mark);
                result
            }
            Node::WhileStmt(w) => {
                self.spawn_walk_node(&w.cond, inner)?;
                if let Some(pattern) = &w.let_pattern {
                    let mark = inner.len();
                    for b in &pattern.bindings {
                        inner.push(b.clone());
                    }
                    let result = self.spawn_walk_block(&w.body, inner);
                    inner.truncate(mark);
                    result
                } else {
                    self.spawn_walk_block(&w.body, inner)
                }
            }
            Node::BlockStmt(b) => self.spawn_walk_block(&b.body, inner),
            Node::ArenaStmt(b) => self.spawn_walk_block(&b.body, inner),
            Node::ExprStmt(e) => self.spawn_walk_node(e, inner),
            Node::ReturnStmt(r) => {
                if let Some(value) = &r.value {
                    self.spawn_walk_node(value, inner)?;
                }
                Ok(())
            }
            Node::ReleaseStmt(r) => {
                // A release is a use of the binding for this guard's
                // purposes: releasing a borrow-typed outer binding
                // from inside the closure captures the borrow.
                if !inner.iter().any(|n| n == &r.var_name) {
                    if let Some(ty) = self.lookup_ty(&r.var_name) {
                        if self.go_type_has_borrow(&ty, 0) {
                            return Err(spawn_escape(format!(
                                "BorrowEscape: thread::spawn: closure releases borrowed value `{}` from the enclosing scope, which would dangle or race across the thread boundary; pass an owned value (such as an Arc clone) or a raw pointer instead",
                                r.var_name
                            )));
                        }
                    }
                }
                Ok(())
            }
            Node::GoStmt(g) => self.spawn_walk_node(&g.call, inner),
            Node::DeferStmt(d) => self.spawn_walk_node(&d.call, inner),
            Node::SelectStmt(s) => {
                for arm in &s.arms {
                    let mark = inner.len();
                    if let Some((binding, closed)) = &arm.pattern {
                        inner.push(binding.clone());
                        if let Some(closed) = closed {
                            inner.push(closed.clone());
                        }
                    }
                    if let Some(recv) = &arm.recv {
                        self.spawn_walk_node(recv, inner)?;
                    }
                    let result = self.spawn_walk_block(&arm.body, inner);
                    inner.truncate(mark);
                    result?;
                }
                if let Some(timeout) = &s.timeout {
                    self.spawn_walk_node(&timeout.expr, inner)?;
                    self.spawn_walk_block(&timeout.body, inner)?;
                }
                Ok(())
            }
            Node::BinaryExpr(b) => {
                self.spawn_walk_node(&b.left, inner)?;
                self.spawn_walk_node(&b.right, inner)
            }
            Node::CallExpr(c) => {
                if let Some(callee) = &c.callee {
                    self.spawn_walk_node(callee, inner)?;
                }
                for arg in &c.args {
                    self.spawn_walk_node(arg, inner)?;
                }
                Ok(())
            }
            Node::FieldExpr(f) => self.spawn_walk_node(&f.expr, inner),
            Node::IndexExpr(i) => {
                self.spawn_walk_node(&i.target, inner)?;
                self.spawn_walk_node(&i.index, inner)
            }
            Node::SliceExpr(s) => {
                self.spawn_walk_node(&s.target, inner)?;
                self.spawn_walk_node(&s.start, inner)?;
                self.spawn_walk_node(&s.end, inner)
            }
            Node::DerefExpr(d) => self.spawn_walk_node(&d.expr, inner),
            Node::CastExpr(c) => self.spawn_walk_node(&c.expr, inner),
            Node::AwaitExpr(a) => self.spawn_walk_node(&a.expr, inner),
            Node::TryExpr(t) => self.spawn_walk_node(&t.expr, inner),
            Node::MoveExpr(m) => self.spawn_walk_node(&m.expr, inner),
            Node::TupleLiteral(t) => {
                for e in &t.elements {
                    self.spawn_walk_node(e, inner)?;
                }
                Ok(())
            }
            Node::ArrayLiteral(a) => {
                for e in &a.elements {
                    self.spawn_walk_node(e, inner)?;
                }
                Ok(())
            }
            Node::RepeatArrayLiteral(r) => self.spawn_walk_node(&r.value, inner),
            Node::StructLiteral(s) => {
                for field in &s.fields {
                    self.spawn_walk_node(&field.value, inner)?;
                }
                if let Some(update) = &s.update_expr {
                    self.spawn_walk_node(update, inner)?;
                }
                Ok(())
            }
            Node::EnumLiteral(e) => {
                for field in &e.fields {
                    self.spawn_walk_node(&field.value, inner)?;
                }
                Ok(())
            }
            Node::UnsafeExpr(u) => self.spawn_walk_block(&u.body, inner),
            Node::Literal(_) | Node::GenericFuncRef(_) | Node::InlineAsmExpr(_) => Ok(()),
            _ => Ok(()),
        }
    }
}
