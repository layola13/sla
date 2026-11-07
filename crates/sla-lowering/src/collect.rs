//! Binding-collection scans, mirrored from lowering_rules.zig
//! `collectRepeatedLetBindings` (line 803) and its counting walker.
//! A name bound by two or more `let` statements in one function body
//! is "repeated" and gets a fresh alias register at each later binding
//! site (codegen's genBlock). The result is membership-only: callers
//! must never iterate it into an emission path (house discipline).

use std::collections::{HashMap, HashSet};

use sla_ast::Node;

/// lowering_rules.zig:803 `collectRepeatedLetBindings`.
pub fn collect_repeated_let_bindings(body: &[Node]) -> HashSet<String> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    count_let_bindings_in_block(body, &mut counts);
    counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .map(|(name, _)| name)
        .collect()
}

fn count_let_bindings_in_block(body: &[Node], counts: &mut HashMap<String, u32>) {
    for node in body {
        count_let_bindings_in_node(node, counts);
    }
}

/// lowering_rules.zig `countLetBindingsInNode`: only `let` statement
/// names are counted; every composite node recurses into its children.
fn count_let_bindings_in_node(node: &Node, counts: &mut HashMap<String, u32>) {
    match node {
        Node::LetStmt(let_stmt) => {
            *counts.entry(let_stmt.name.clone()).or_insert(0) += 1;
            count_let_bindings_in_node(&let_stmt.value, counts);
        }
        Node::LetElseStmt(let_else) => {
            count_let_bindings_in_node(&let_else.value, counts);
            count_let_bindings_in_block(&let_else.else_block, counts);
        }
        Node::LetDestructureStmt(let_stmt) => {
            count_let_bindings_in_node(&let_stmt.value, counts);
        }
        Node::ConstStmt(c) => count_let_bindings_in_node(&c.value, counts),
        Node::AssignStmt(assign) => {
            count_let_bindings_in_node(&assign.target, counts);
            count_let_bindings_in_node(&assign.value, counts);
        }
        Node::ExprStmt(expr) => count_let_bindings_in_node(expr, counts),
        Node::ReturnStmt(ret) => {
            if let Some(value) = &ret.value {
                count_let_bindings_in_node(value, counts);
            }
        }
        Node::BlockStmt(block) => count_let_bindings_in_block(&block.body, counts),
        Node::BinaryExpr(bin) => {
            count_let_bindings_in_node(&bin.left, counts);
            count_let_bindings_in_node(&bin.right, counts);
        }
        Node::CallExpr(call) => {
            for arg in &call.args {
                count_let_bindings_in_node(arg, counts);
            }
        }
        Node::FieldExpr(field) => count_let_bindings_in_node(&field.expr, counts),
        Node::StructLiteral(lit) => {
            for field in &lit.fields {
                count_let_bindings_in_node(&field.value, counts);
            }
            if let Some(update) = &lit.update_expr {
                count_let_bindings_in_node(update, counts);
            }
        }
        Node::EnumLiteral(lit) => {
            for field in &lit.fields {
                count_let_bindings_in_node(&field.value, counts);
            }
        }
        Node::TupleLiteral(lit) => {
            for elem in &lit.elements {
                count_let_bindings_in_node(elem, counts);
            }
        }
        Node::ArrayLiteral(lit) => {
            for elem in &lit.elements {
                count_let_bindings_in_node(elem, counts);
            }
        }
        Node::RepeatArrayLiteral(lit) => count_let_bindings_in_node(&lit.value, counts),
        Node::IndexExpr(idx) => {
            count_let_bindings_in_node(&idx.target, counts);
            count_let_bindings_in_node(&idx.index, counts);
        }
        Node::SliceExpr(slc) => {
            count_let_bindings_in_node(&slc.target, counts);
            count_let_bindings_in_node(&slc.start, counts);
            count_let_bindings_in_node(&slc.end, counts);
        }
        Node::IfExpr(ife) => {
            count_let_bindings_in_node(&ife.cond, counts);
            if let Some(chain) = &ife.let_chain {
                for cond in chain {
                    count_let_bindings_in_node(&cond.value, counts);
                }
            }
            count_let_bindings_in_block(&ife.then_block, counts);
            if let Some(else_block) = &ife.else_block {
                count_let_bindings_in_block(else_block, counts);
            }
        }
        Node::SwitchExpr(swe) => {
            count_let_bindings_in_node(&swe.val, counts);
            for case in &swe.cases {
                count_let_bindings_in_block(&case.body, counts);
            }
        }
        Node::MatchExpr(mat) => {
            count_let_bindings_in_node(&mat.val, counts);
            for case in &mat.cases {
                if let Some(guard) = &case.guard {
                    count_let_bindings_in_node(guard, counts);
                }
                count_let_bindings_in_block(&case.body, counts);
            }
        }
        Node::WhileStmt(w) => {
            count_let_bindings_in_node(&w.cond, counts);
            count_let_bindings_in_block(&w.body, counts);
        }
        Node::ForStmt(f) => {
            count_let_bindings_in_node(&f.start, counts);
            if let Some(end) = &f.end {
                count_let_bindings_in_node(end, counts);
            }
            count_let_bindings_in_block(&f.body, counts);
        }
        Node::BorrowExpr(borrow) => count_let_bindings_in_node(&borrow.expr, counts),
        Node::MoveExpr(mv) => count_let_bindings_in_node(&mv.expr, counts),
        Node::DerefExpr(deref) => count_let_bindings_in_node(&deref.expr, counts),
        Node::CastExpr(cast) => count_let_bindings_in_node(&cast.expr, counts),
        Node::UnsafeExpr(ue) => count_let_bindings_in_block(&ue.body, counts),
        Node::AwaitExpr(aw) => count_let_bindings_in_node(&aw.expr, counts),
        Node::TryExpr(t) => count_let_bindings_in_node(&t.expr, counts),
        Node::ClosureLiteral(closure) => count_let_bindings_in_node(&closure.body, counts),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{LetStmt, Literal};

    fn let_node(name: &str) -> Node {
        Node::LetStmt(LetStmt {
            name: name.to_string(),
            ty: None,
            value: Box::new(Node::Literal(Literal::IntVal(1))),
        })
    }

    #[test]
    fn repeated_lets_detected() {
        let body = vec![let_node("a"), let_node("b"), let_node("a")];
        let repeated = collect_repeated_let_bindings(&body);
        assert!(repeated.contains("a"));
        assert!(!repeated.contains("b"));
    }
}
