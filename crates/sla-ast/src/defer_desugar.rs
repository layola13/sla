//! Pre-check desugar for `defer` (goroutine syntax batch P3),
//! a direct port of the Zig plugin's `defer_desugar.zig`.
//!
//! Runs in the CLI front end after import expansion and before
//! the prunes / checker, so the checker and both codegens never
//! see a `DeferStmt` (any survivor is a loud error downstream).
//!
//! Semantics (DESIGN section 5): a defer registers its call for
//! the enclosing BLOCK. Arguments are snapshotted into synthetic
//! lets at the defer statement — Go's evaluate-at-defer rule —
//! and copies of the call run, last-in first-out, at every block
//! exit the desugar can see: block fallthrough, `return`, and
//! `break`/`continue` crossing the block. Defers do NOT run on
//! `?`-propagation exits or on panic/trap.
//!
//! Macro bodies are transformed as independent root scopes (a
//! documented approximation: a defer inside a macro body binds
//! to the macro body's own blocks, not the expansion site's).

use crate::node::Node;
use crate::stmts::{LetStmt, ReturnStmt};

type DeferList = Vec<Node>;

pub struct Desugarer {
    counter: usize,
}

impl Default for Desugarer {
    fn default() -> Self {
        Self::new()
    }
}

impl Desugarer {
    pub fn new() -> Self {
        Desugarer { counter: 0 }
    }

    fn fresh_name(&mut self, prefix: &str) -> String {
        self.counter += 1;
        format!("{}{}", prefix, self.counter)
    }

    pub fn desugar_program(&mut self, program: &mut crate::decls::Program) -> Result<(), String> {
        for decl in &mut program.decls {
            match decl {
                Node::FuncDecl(f) => {
                    let body = std::mem::take(&mut f.body);
                    f.body = self.process_block(body, &[])?;
                }
                Node::TestDecl(t) => {
                    let body = std::mem::take(&mut t.body);
                    t.body = self.process_block(body, &[])?;
                }
                Node::ImplDecl(imp) => {
                    for method in &mut imp.methods {
                        if let Node::FuncDecl(f) = method {
                            let body = std::mem::take(&mut f.body);
                            f.body = self.process_block(body, &[])?;
                        }
                    }
                }
                Node::MacroDecl(m) => {
                    let body = std::mem::take(&mut m.body);
                    m.body = self.process_block(body, &[])?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Process one statement list. `outer` is the chain of defer
    /// lists of enclosing blocks within the current loop body (or
    /// the whole function outside loops), innermost last; `return`
    /// runs the whole chain, break/continue run the whole chain
    /// too (the chain is reset at loop boundaries by the caller).
    fn process_block(
        &mut self,
        stmts: Vec<Node>,
        outer: &[DeferList],
    ) -> Result<Vec<Node>, String> {
        let mut out: Vec<Node> = Vec::new();
        let mut defers: DeferList = Vec::new();
        let mut last_was_exit = false;
        for mut stmt in stmts {
            last_was_exit = false;
            if let Node::DeferStmt(d) = &stmt {
                let call = (*d.call).clone();
                let snapshot = self.snapshot_call(&call, &mut out)?;
                defers.push(snapshot);
                continue;
            }
            if let Node::ReturnStmt(ret) = &stmt {
                let has_defers = !defers.is_empty() || outer.iter().any(|l| !l.is_empty());
                if has_defers {
                    let mut ret_value = ret.value.clone();
                    if let Some(value) = &ret_value {
                        if !matches!(**value, Node::Identifier(_) | Node::Literal(_)) {
                            let name = self.fresh_name("sla_dfret_");
                            out.push(Node::LetStmt(LetStmt {
                                name: name.clone(),
                                ty: None,
                                value: Box::new((**value).clone()),
                            }));
                            ret_value = Some(Box::new(Node::Identifier(name)));
                        }
                    }
                    self.emit_defers(&mut out, outer, &defers)?;
                    out.push(Node::ReturnStmt(ReturnStmt { value: ret_value }));
                } else {
                    out.push(stmt);
                }
                last_was_exit = true;
                continue;
            }
            if matches!(stmt, Node::BreakStmt(_) | Node::ContinueStmt(_)) {
                self.emit_defers(&mut out, outer, &defers)?;
                out.push(stmt);
                last_was_exit = true;
                continue;
            }
            let mut chain: Vec<DeferList> = outer.to_vec();
            chain.push(defers.clone());
            self.process_stmt_children(&mut stmt, &chain)?;
            out.push(stmt);
        }
        // Block fallthrough: this block's own defers run (enclosing
        // blocks run at their own exits). Skipped when the last
        // statement already exited the block.
        if !last_was_exit {
            self.emit_defers(&mut out, &[], &defers)?;
        }
        Ok(out)
    }

    /// Emit copies of every defer in the chain, innermost block
    /// first, LIFO within each block.
    fn emit_defers(
        &mut self,
        out: &mut Vec<Node>,
        outer: &[DeferList],
        own: &DeferList,
    ) -> Result<(), String> {
        let mut chain: Vec<&DeferList> = outer.iter().collect();
        chain.push(own);
        for list in chain.iter().rev() {
            for call in list.iter().rev() {
                out.push(Node::ExprStmt(Box::new(clone_expr(call)?)));
            }
        }
        Ok(())
    }

    /// Snapshot a defer call's arguments at the defer statement:
    /// each argument becomes a synthetic let, and the registered
    /// call is rebuilt over the snapshot identifiers.
    fn snapshot_call(&mut self, call_node: &Node, out: &mut Vec<Node>) -> Result<Node, String> {
        let call = match call_node {
            Node::CallExpr(c) => c,
            _ => return Err("defer operand is not a call expression".to_string()),
        };
        let mut new_args: Vec<Node> = Vec::new();
        for arg in &call.args {
            let name = self.fresh_name("sla_dfa_");
            out.push(Node::LetStmt(LetStmt {
                name: name.clone(),
                ty: None,
                value: Box::new(arg.clone()),
            }));
            new_args.push(Node::Identifier(name));
        }
        let mut rebuilt = call.clone();
        rebuilt.args = new_args;
        if let Some(callee) = &call.callee {
            rebuilt.callee = Some(Box::new(clone_expr(callee)?));
        }
        Ok(Node::CallExpr(rebuilt))
    }

    /// Recurse into a statement's nested blocks and expressions.
    /// Loop bodies reset the defer chain: a break inside the loop
    /// crosses only blocks within the loop body; defers registered
    /// outside the loop run when the loop's enclosing block exits.
    fn process_stmt_children(
        &mut self,
        stmt: &mut Node,
        chain: &[DeferList],
    ) -> Result<(), String> {
        match stmt {
            Node::BlockStmt(b) | Node::ArenaStmt(b) => {
                let body = std::mem::take(&mut b.body);
                b.body = self.process_block(body, chain)?;
            }
            Node::WhileStmt(w) => {
                self.process_expr(&mut w.cond, chain)?;
                let body = std::mem::take(&mut w.body);
                w.body = self.process_block(body, &[])?;
            }
            Node::ForStmt(f) => {
                self.process_expr(&mut f.start, chain)?;
                if let Some(end) = &mut f.end {
                    self.process_expr(end, chain)?;
                }
                let body = std::mem::take(&mut f.body);
                f.body = self.process_block(body, &[])?;
            }
            Node::LetStmt(l) => self.process_expr(&mut l.value, chain)?,
            Node::LetElseStmt(l) => {
                self.process_expr(&mut l.value, chain)?;
                let body = std::mem::take(&mut l.else_block);
                l.else_block = self.process_block(body, chain)?;
            }
            Node::LetDestructureStmt(l) => self.process_expr(&mut l.value, chain)?,
            Node::ConstStmt(c) => self.process_expr(&mut c.value, chain)?,
            Node::AssignStmt(a) => {
                self.process_expr(&mut a.target, chain)?;
                self.process_expr(&mut a.value, chain)?;
            }
            Node::ReturnStmt(r) => {
                if let Some(value) = &mut r.value {
                    self.process_expr(value, chain)?;
                }
            }
            Node::ExprStmt(e) => self.process_expr(e, chain)?,
            _ => {}
        }
        Ok(())
    }

    /// Process statement lists nested inside an expression (if/match
    /// bodies and friends). Defers registered there bind to those
    /// blocks; returns and breaks inside them cross the expression
    /// block first, so the chain passes through.
    fn process_expr(&mut self, expr: &mut Node, chain: &[DeferList]) -> Result<(), String> {
        match expr {
            Node::IfExpr(e) => {
                self.process_expr(&mut e.cond, chain)?;
                if let Some(conds) = &mut e.let_chain {
                    for cond in conds {
                        self.process_expr(&mut cond.value, chain)?;
                    }
                }
                let then = std::mem::take(&mut e.then_block);
                e.then_block = self.process_block(then, chain)?;
                if let Some(else_block) = &mut e.else_block {
                    let body = std::mem::take(else_block);
                    *else_block = self.process_block(body, chain)?;
                }
            }
            Node::SwitchExpr(e) => {
                self.process_expr(&mut e.val, chain)?;
                for case in &mut e.cases {
                    let body = std::mem::take(&mut case.body);
                    case.body = self.process_block(body, chain)?;
                }
            }
            Node::MatchExpr(e) => {
                self.process_expr(&mut e.val, chain)?;
                for case in &mut e.cases {
                    if let Some(guard) = &mut case.guard {
                        self.process_expr(guard, chain)?;
                    }
                    let body = std::mem::take(&mut case.body);
                    case.body = self.process_block(body, chain)?;
                }
            }
            Node::UnsafeExpr(e) => {
                let body = std::mem::take(&mut e.body);
                e.body = self.process_block(body, chain)?;
            }
            Node::ClosureLiteral(c) => {
                // A defer inside a closure body binds to the
                // closure's own scope chain.
                self.process_expr(&mut c.body, &[])?;
            }
            Node::CallExpr(c) => {
                if let Some(callee) = &mut c.callee {
                    self.process_expr(callee, chain)?;
                }
                for arg in &mut c.args {
                    self.process_expr(arg, chain)?;
                }
            }
            Node::BinaryExpr(b) => {
                self.process_expr(&mut b.left, chain)?;
                self.process_expr(&mut b.right, chain)?;
            }
            Node::AwaitExpr(a) => self.process_expr(&mut a.expr, chain)?,
            Node::TryExpr(t) => self.process_expr(&mut t.expr, chain)?,
            Node::BorrowExpr(b) => self.process_expr(&mut b.expr, chain)?,
            Node::MoveExpr(m) => self.process_expr(&mut m.expr, chain)?,
            Node::DerefExpr(d) => self.process_expr(&mut d.expr, chain)?,
            Node::CastExpr(c) => self.process_expr(&mut c.expr, chain)?,
            Node::FieldExpr(f) => self.process_expr(&mut f.expr, chain)?,
            Node::StructLiteral(s) => {
                for field in &mut s.fields {
                    self.process_expr(&mut field.value, chain)?;
                }
                if let Some(update) = &mut s.update_expr {
                    self.process_expr(update, chain)?;
                }
            }
            Node::EnumLiteral(e) => {
                for field in &mut e.fields {
                    self.process_expr(&mut field.value, chain)?;
                }
            }
            Node::TupleLiteral(t) => {
                for elem in &mut t.elements {
                    self.process_expr(elem, chain)?;
                }
            }
            Node::ArrayLiteral(a) => {
                for elem in &mut a.elements {
                    self.process_expr(elem, chain)?;
                }
            }
            Node::RepeatArrayLiteral(r) => self.process_expr(&mut r.value, chain)?,
            Node::IndexExpr(i) => {
                self.process_expr(&mut i.target, chain)?;
                self.process_expr(&mut i.index, chain)?;
            }
            Node::SliceExpr(s) => {
                self.process_expr(&mut s.target, chain)?;
                self.process_expr(&mut s.start, chain)?;
                self.process_expr(&mut s.end, chain)?;
            }
            _ => {}
        }
        Ok(())
    }
}

/// Deep-copy an expression node (defer call templates are emitted
/// once per exit path). Mirrors the Zig `cloneExpr` kind list;
/// any other shape is a loud failure, as in the Zig pass.
fn clone_expr(node: &Node) -> Result<Node, String> {
    match node {
        Node::Literal(_)
        | Node::Identifier(_)
        | Node::GenericFuncRef(_)
        | Node::CallExpr(_)
        | Node::BinaryExpr(_)
        | Node::BorrowExpr(_)
        | Node::MoveExpr(_)
        | Node::DerefExpr(_)
        | Node::CastExpr(_)
        | Node::FieldExpr(_)
        | Node::IndexExpr(_)
        | Node::SliceExpr(_)
        | Node::TupleLiteral(_)
        | Node::ArrayLiteral(_)
        | Node::RepeatArrayLiteral(_)
        | Node::StructLiteral(_)
        | Node::EnumLiteral(_)
        | Node::TryExpr(_)
        | Node::AwaitExpr(_) => Ok(node.clone()),
        _ => Err(
            "defer: argument expression of this shape cannot be deferred in this batch"
                .to_string(),
        ),
    }
}

/// Desugar every function/test/impl-method/macro body in a parsed
/// program (the pipeline entry point).
pub fn desugar_program(root: &mut Node) -> Result<(), String> {
    if let Node::Program(program) = root {
        Desugarer::new().desugar_program(program)?;
    }
    Ok(())
}
