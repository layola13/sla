//! R-3 (J-5): `@check_hook` validation and the
//! `macro_param_audit` read-only audit.
//!
//! The hook attaches per file. A name that is not in the
//! registry is `UnknownCheckHook` (never a silent no-op).
//! The audit walks every macro in the file: each parameter
//! must be REFERENCED by the body under lexical scoping —
//! a reference binds to the innermost local declaration, so
//! a body `let x` shadows parameter `x` from its declaration
//! point on, and references after it do not count for the
//! parameter. The leading output parameter counts as
//! referenced when it is assigned (its assignment IS its
//! use). An unreferenced parameter is `MacroParamUnused`.

use std::collections::HashSet;

use sla_ast::node::Node;
use sla_ast::Program;

use crate::diag::{CheckResult, Diagnostic, ErrorKind};

/// The registered hook names (J-5 ships exactly one).
pub const KNOWN_HOOKS: &[&str] = &["macro_param_audit"];

impl crate::checker::Checker {
    /// Validate the file's hook declarations and, when the
    /// audit hook is attached, run it over the program's
    /// macros. Called from check_program before bodies are
    /// checked so the diagnostic precedes body faults.
    pub fn apply_check_hooks(&mut self, program: &Program) -> CheckResult<()> {
        let mut audit = false;
        for decl in &program.decls {
            if let Node::CheckHookDecl(hook) = decl {
                if !KNOWN_HOOKS.contains(&hook.name.as_str()) {
                    return Err(Diagnostic::new(
                        ErrorKind::UnknownCheckHook,
                        format!(
                            "UnknownCheckHook: `{}` is not a registered check hook",
                            hook.name
                        ),
                    ));
                }
                if hook.name == "macro_param_audit" {
                    audit = true;
                }
            }
        }
        if audit {
            for decl in &program.decls {
                if let Node::MacroDecl(m) = decl {
                    audit_macro_params(m)?;
                }
            }
        }
        Ok(())
    }
}

/// Does this statement list reference `param` under lexical
/// scoping? `shadowed` accumulates names declared by `let` /
/// `let`-destructuring as the walk proceeds in order; once
/// `param` is shadowed, later identifier hits bind to the
/// local and no longer count.
fn body_references(body: &[Node], param: &str, is_output: bool) -> bool {
    let mut shadowed: HashSet<String> = HashSet::new();
    body.iter().any(|n| node_references(n, param, is_output, &mut shadowed))
}

fn node_references(
    node: &Node,
    param: &str,
    is_output: bool,
    shadowed: &mut HashSet<String>,
) -> bool {
    match node {
        Node::Identifier(name) => name == param && !shadowed.contains(name),
        Node::LetStmt(s) => {
            // The initializer is evaluated before the binding
            // exists; then the name shadows from here on.
            let hit = node_references(&s.value, param, is_output, shadowed);
            shadowed.insert(s.name.clone());
            hit
        }
        Node::LetDestructureStmt(s) => {
            let hit = node_references(&s.value, param, is_output, shadowed);
            for n in &s.names {
                shadowed.insert(n.clone());
            }
            if let Some(r) = &s.rest_name {
                shadowed.insert(r.clone());
            }
            hit
        }
        Node::AssignStmt(s) => {
            // An assignment to the leading output parameter is
            // its defining use.
            if is_output {
                if let Node::Identifier(t) = &*s.target {
                    if t == param && !shadowed.contains(t) {
                        return true;
                    }
                }
            }
            node_references(&s.target, param, is_output, shadowed)
                || node_references(&s.value, param, is_output, shadowed)
        }
        _ => node_children(node)
            .into_iter()
            .any(|c| node_references(c, param, is_output, shadowed)),
    }
}

fn audit_macro_params(m: &sla_ast::MacroDecl) -> CheckResult<()> {
    for (i, param) in m.params.iter().enumerate() {
        let is_output = i == 0;
        if !body_references(&m.body, param, is_output) {
            return Err(Diagnostic::new(
                ErrorKind::MacroParamUnused,
                format!(
                    "MacroParamUnused: parameter `{param}` of macro `{}` is never referenced in its body",
                    m.name
                ),
            ));
        }
    }
    Ok(())
}
/// All direct child nodes of `node` (a generic structural
/// walk; statement-level binding forms are handled by the
/// caller-visible arms in `node_references`). Field names
/// follow the sla-ast struct definitions exactly.
fn node_children(node: &Node) -> Vec<&Node> {
    let mut out: Vec<&Node> = Vec::new();
    match node {
        Node::BlockStmt(b) | Node::ArenaStmt(b) => {
            out.extend(b.body.iter());
        }
        Node::ExprStmt(e) => out.push(e),
        Node::ReturnStmt(r) => {
            if let Some(v) = &r.value {
                out.push(v);
            }
        }
        Node::LetStmt(s) => out.push(&s.value),
        Node::LetElseStmt(s) => {
            out.push(&s.value);
            out.extend(s.else_block.iter());
        }
        Node::LetDestructureStmt(s) => out.push(&s.value),
        Node::ConstStmt(s) => out.push(&s.value),
        Node::AssignStmt(s) => {
            out.push(&s.target);
            out.push(&s.value);
        }
        Node::IfExpr(e) => {
            out.push(&e.cond);
            out.extend(e.then_block.iter());
            if let Some(b) = &e.else_block {
                out.extend(b.iter());
            }
        }
        Node::WhileStmt(w) => {
            out.push(&w.cond);
            out.extend(w.body.iter());
        }
        Node::ForStmt(f) => {
            out.push(&f.start);
            if let Some(e) = &f.end {
                out.push(e);
            }
            out.extend(f.body.iter());
        }
        Node::CallExpr(c) => {
            out.extend(c.args.iter());
            if let Some(callee) = &c.callee {
                out.push(callee);
            }
        }
        Node::BinaryExpr(b) => {
            out.push(&b.left);
            out.push(&b.right);
        }
        Node::FieldExpr(f) => out.push(&f.expr),
        Node::IndexExpr(i) => {
            out.push(&i.target);
            out.push(&i.index);
        }
        Node::SliceExpr(s) => {
            out.push(&s.target);
            out.push(&s.start);
            out.push(&s.end);
        }
        Node::CastExpr(c) => out.push(&c.expr),
        Node::MoveExpr(m) => out.push(&m.expr),
        Node::BorrowExpr(b) => out.push(&b.expr),
        Node::DerefExpr(d) => out.push(&d.expr),
        Node::AwaitExpr(a) => out.push(&a.expr),
        Node::TryExpr(t) => out.push(&t.expr),
        Node::UnsafeExpr(u) => out.extend(u.body.iter()),
        Node::StructLiteral(s) => {
            out.extend(s.fields.iter().map(|f| &*f.value));
            if let Some(u) = &s.update_expr {
                out.push(u);
            }
        }
        Node::EnumLiteral(e) => {
            out.extend(e.fields.iter().map(|f| &*f.value));
        }
        Node::ArrayLiteral(a) => out.extend(a.elements.iter()),
        Node::TupleLiteral(t) => out.extend(t.elements.iter()),
        Node::RepeatArrayLiteral(a) => out.push(&a.value),
        Node::ClosureLiteral(c) => out.push(&c.body),
        Node::SwitchExpr(s) => {
            out.push(&s.val);
            for case in &s.cases {
                out.push(&case.pattern);
                out.extend(case.body.iter());
            }
        }
        Node::MatchExpr(m) => {
            out.push(&m.val);
            for case in &m.cases {
                if let Some(g) = &case.guard {
                    out.push(g);
                }
                out.extend(case.body.iter());
            }
        }
        Node::GoStmt(g) => out.push(&g.call),
        Node::DeferStmt(d) => out.push(&d.call),
        Node::SelectStmt(s) => {
            for arm in &s.arms {
                if let Some(r) = &arm.recv {
                    out.push(r);
                }
                out.extend(arm.body.iter());
            }
        }
        _ => {}
    }
    out
}
