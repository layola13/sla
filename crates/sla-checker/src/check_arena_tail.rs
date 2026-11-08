//! Arena tail-expression checking split out of check_arena.rs
//! for the 700-line file cap (body unchanged).

use std::cell::RefCell;
use std::collections::HashSet;
use sla_ast::exprs::CallExpr;
use sla_ast::node::Node;
use sla_ast::types::Type;
use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};

impl Checker {
    /// Tail-expression form of route 1: `fn f() -> T { arena { .. v } }`
    /// — the block's tail value flows out as the function result.
    pub fn arena_check_tail(&mut self, tail: &Node) -> CheckResult<()> {
        self.arena_check_return(Some(tail))
    }
}
