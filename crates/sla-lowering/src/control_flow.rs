//! Control-flow facts, mirrored from lowering_rules.zig (termination
//! and loop-control scans, lines 796-1109) and control_flow_rules.zig
//! (function-exit cleanup planning, lines 23-45). lowering_rules.zig
//! re-exports the latter as its own (`planFunctionTailCleanup`,
//! lowering_rules.zig:6120-6121), so both live here.

use sla_ast::Node;


// ---------------------------------------------------------------------
// Termination (lowering_rules.zig:985-1109)
// ---------------------------------------------------------------------

/// lowering_rules.zig:985 `blockTerminates`: only the LAST statement of
/// a block decides.
pub fn block_terminates(block: &[Node]) -> bool {
    match block.last() {
        None => false,
        Some(last) => stmt_terminates(last),
    }
}

/// lowering_rules.zig:990 `stmtTerminates`.
pub fn stmt_terminates(stmt: &Node) -> bool {
    match stmt {
        Node::ReturnStmt(_) | Node::BreakStmt(_) | Node::ContinueStmt(_) => true,
        Node::ExprStmt(expr) => expr_terminates(expr),
        _ => false,
    }
}

/// lowering_rules.zig:1076 `exprTerminates` (private there; exported
/// here because both emitters' statement layer needs the same fact).
pub fn expr_terminates(expr: &Node) -> bool {
    match expr {
        Node::CallExpr(call) => call.func_name == "panic",
        Node::UnsafeExpr(ue) => block_terminates(&ue.body),
        Node::IfExpr(ife) => {
            block_terminates(&ife.then_block)
                && match &ife.else_block {
                    Some(eb) => block_terminates(eb),
                    None => false,
                }
        }
        Node::MatchExpr(mat) => {
            if mat.cases.is_empty() {
                return false;
            }
            mat.cases.iter().all(|case| block_terminates(&case.body))
        }
        Node::SwitchExpr(swe) => {
            // #7: all-diverging switch tails need a `default` arm.
            if swe.cases.is_empty() {
                return false;
            }
            let mut has_default = false;
            for case in &swe.cases {
                if crate::names::is_switch_default_pattern(&case.pattern) {
                    has_default = true;
                }
                if !block_terminates(&case.body) {
                    return false;
                }
            }
            has_default
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------
// Loop control (lowering_rules.zig:796-801, 1000-1075)
// ---------------------------------------------------------------------

/// lowering_rules.zig `LoopControlPlan`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopControlPlan {
    pub has_break: bool,
    pub has_continue: bool,
}

/// lowering_rules.zig:796 `planLoopControl`.
pub fn plan_loop_control(body: &[Node]) -> LoopControlPlan {
    LoopControlPlan {
        has_break: block_contains_current_loop_break(body),
        has_continue: block_contains_current_loop_continue(body),
    }
}

/// lowering_rules.zig:1000 `blockContainsCurrentLoopBreak`. "Current
/// loop" means nested loops shield their own break/continue.
pub fn block_contains_current_loop_break(block: &[Node]) -> bool {
    block.iter().any(stmt_contains_current_loop_break)
}

/// lowering_rules.zig:1007 `blockContainsCurrentLoopContinue`.
pub fn block_contains_current_loop_continue(block: &[Node]) -> bool {
    block.iter().any(stmt_contains_current_loop_continue)
}

fn stmt_contains_current_loop_break(stmt: &Node) -> bool {
    match stmt {
        Node::BreakStmt(_) => true,
        Node::ForStmt(_) | Node::WhileStmt(_) => false,
        Node::BlockStmt(blk) => block_contains_current_loop_break(&blk.body),
        Node::LetElseStmt(let_else) => block_contains_current_loop_break(&let_else.else_block),
        Node::ExprStmt(expr) => expr_contains_current_loop_break(expr),
        _ => false,
    }
}

fn stmt_contains_current_loop_continue(stmt: &Node) -> bool {
    match stmt {
        Node::ContinueStmt(_) => true,
        Node::ForStmt(_) | Node::WhileStmt(_) => false,
        Node::BlockStmt(blk) => block_contains_current_loop_continue(&blk.body),
        Node::LetElseStmt(let_else) => block_contains_current_loop_continue(&let_else.else_block),
        Node::ExprStmt(expr) => expr_contains_current_loop_continue(expr),
        _ => false,
    }
}

fn expr_contains_current_loop_break(expr: &Node) -> bool {
    match expr {
        Node::IfExpr(ife) => {
            block_contains_current_loop_break(&ife.then_block)
                || match &ife.else_block {
                    Some(eb) => block_contains_current_loop_break(eb),
                    None => false,
                }
        }
        Node::SwitchExpr(swe) => swe.cases.iter().any(|case| block_contains_current_loop_break(&case.body)),
        Node::MatchExpr(mat) => mat.cases.iter().any(|case| block_contains_current_loop_break(&case.body)),
        Node::UnsafeExpr(ue) => block_contains_current_loop_break(&ue.body),
        _ => false,
    }
}

fn expr_contains_current_loop_continue(expr: &Node) -> bool {
    match expr {
        Node::IfExpr(ife) => {
            block_contains_current_loop_continue(&ife.then_block)
                || match &ife.else_block {
                    Some(eb) => block_contains_current_loop_continue(eb),
                    None => false,
                }
        }
        Node::SwitchExpr(swe) => swe
            .cases
            .iter()
            .any(|case| block_contains_current_loop_continue(&case.body)),
        Node::MatchExpr(mat) => mat
            .cases
            .iter()
            .any(|case| block_contains_current_loop_continue(&case.body)),
        Node::UnsafeExpr(ue) => block_contains_current_loop_continue(&ue.body),
        _ => false,
    }
}

// ---------------------------------------------------------------------
// Function-exit cleanup (control_flow_rules.zig:23-45)
// ---------------------------------------------------------------------

/// control_flow_rules.zig:23 `FunctionExitCleanupAction`, re-exported
/// by lowering_rules.zig:6120 as `FunctionTailCleanupAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionTailCleanupAction {
    Release,
    TransferResult,
}

impl FunctionTailCleanupAction {
    /// control_flow_rules.zig `releasesValue`.
    pub fn releases_value(self) -> bool {
        self == FunctionTailCleanupAction::Release
    }

    /// control_flow_rules.zig `transfersResult`.
    pub fn transfers_result(self) -> bool {
        self == FunctionTailCleanupAction::TransferResult
    }
}

/// control_flow_rules.zig:36 `planFunctionExitCleanup`, re-exported by
/// lowering_rules.zig:6121 as `planFunctionTailCleanup`. The result root
/// here is ONLY an identifier or a `^identifier` move (unlike
/// `root_identifier`, no field/index/borrow peeling).
pub fn plan_function_tail_cleanup(
    cleanup_name: &str,
    result_expr: &Node,
) -> FunctionTailCleanupAction {
    let result_name = match result_expr {
        Node::Identifier(name) => name.as_str(),
        Node::MoveExpr(mv) => match &*mv.expr {
            Node::Identifier(name) => name.as_str(),
            _ => return FunctionTailCleanupAction::Release,
        },
        _ => return FunctionTailCleanupAction::Release,
    };
    if cleanup_name == result_name {
        FunctionTailCleanupAction::TransferResult
    } else {
        FunctionTailCleanupAction::Release
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::root_identifier;
    use sla_ast::{BinaryExpr, BinaryOp, ReturnStmt};

    fn ident(name: &str) -> Node {
        Node::Identifier(name.to_string())
    }

    #[test]
    fn tail_cleanup_matches_zig_unit_tests() {
        // lowering_rules.zig:8751-8755 expectations.
        let result = ident("result");
        assert_eq!(
            plan_function_tail_cleanup("result", &result),
            FunctionTailCleanupAction::TransferResult
        );
        let moved = Node::MoveExpr(sla_ast::MoveExpr { expr: Box::new(ident("result")) });
        assert_eq!(
            plan_function_tail_cleanup("result", &moved),
            FunctionTailCleanupAction::TransferResult
        );
        assert_eq!(
            plan_function_tail_cleanup("other", &result),
            FunctionTailCleanupAction::Release
        );
        let other = ident("other");
        assert_eq!(
            plan_function_tail_cleanup("result", &other),
            FunctionTailCleanupAction::Release
        );
        let field = Node::FieldExpr(sla_ast::FieldExpr {
            expr: Box::new(ident("result")),
            field_name: "x".to_string(),
        });
        assert_eq!(
            plan_function_tail_cleanup("result", &field),
            FunctionTailCleanupAction::Release
        );
    }

    #[test]
    fn termination_facts() {
        assert!(!block_terminates(&[]));
        let ret = Node::ReturnStmt(ReturnStmt { value: None });
        assert!(block_terminates(&[ret]));
        let bin = Node::BinaryExpr(BinaryExpr {
            op: BinaryOp::Add,
            left: Box::new(ident("a")),
            right: Box::new(ident("b")),
        });
        assert!(!expr_terminates(&bin));
        assert!(root_identifier(&bin).is_none());
    }
}
