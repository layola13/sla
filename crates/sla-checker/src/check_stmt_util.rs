//! Statement-check leaf helpers split out of check_stmt.rs
//! for the 700-line file cap (bodies unchanged).

use sla_ast::node::Node;

/// Root identifier of an expression through borrow/move wrappers.
pub fn root_identifier(node: &Node) -> Option<&str> {
    match node {
        Node::Identifier(name) => Some(name.as_str()),
        Node::BorrowExpr(b) => root_identifier(&b.expr),
        Node::MoveExpr(m) => root_identifier(&m.expr),
        _ => None,
    }
}
