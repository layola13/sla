//! Faithful port of `control_flow_rules.zig` (276 lines): the pure
//! decision rules the Zig checker consults for branch state merges,
//! function-exit cleanup planning, and macro parameter consumption.
//!
//! These functions are pure in Zig too (no checker state), so this file
//! is the one place where the port can be line-for-line; the stateful
//! machinery that *applies* the plans lives in checker.rs / scope.rs,
//! and that is exactly where the two implementations diverge (D-C1).

use sla_ast::node::Node;

/// Mirrors Zig `ValueState` (control_flow_rules.zig:192).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueState {
    Uninitialized,
    Active,
    Consumed,
}

/// Mirrors Zig `BranchStateMergeAction` (control_flow_rules.zig:8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchStateMergeAction {
    RestorePre,
    RestoreThen,
    RestoreElse,
    KeepCurrent,
}

/// Mirrors Zig `planBranchStateMerge` (control_flow_rules.zig:15).
pub fn plan_branch_state_merge(
    then_terminates: bool,
    else_terminates: bool,
) -> BranchStateMergeAction {
    if then_terminates && else_terminates {
        return BranchStateMergeAction::RestorePre;
    }
    if then_terminates {
        return BranchStateMergeAction::RestoreElse;
    }
    if else_terminates {
        return BranchStateMergeAction::RestoreThen;
    }
    BranchStateMergeAction::KeepCurrent
}

/// Mirrors Zig `FunctionExitCleanupAction` (control_flow_rules.zig:30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionExitCleanupAction {
    Release,
    TransferResult,
}

/// Root identifier of a result expression, peeling `move_expr`
/// (mirrors the lookup half of Zig `functionExitBindingEscapes` /
/// `cleanupActionForFunctionExit`, control_flow_rules.zig:37-61).
pub fn result_root_identifier(expr: &Node) -> Option<&str> {
    match expr {
        Node::Identifier(name) => Some(name.as_str()),
        Node::MoveExpr(m) => result_root_identifier(&m.expr),
        _ => None,
    }
}

/// Mirrors Zig `planFunctionExitCleanup` (control_flow_rules.zig:44):
/// a binding whose value escapes through the return expression is
/// transferred, everything else is released.
pub fn plan_function_exit_cleanup(
    cleanup_name: &str,
    result_expr: Option<&Node>,
) -> FunctionExitCleanupAction {
    if let Some(expr) = result_expr {
        if let Some(root) = result_root_identifier(expr) {
            if root == cleanup_name {
                return FunctionExitCleanupAction::TransferResult;
            }
        }
    }
    FunctionExitCleanupAction::Release
}

/// Mirrors Zig `MultiBranchStateMergeAction`
/// (control_flow_rules.zig:198).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiBranchStateMergeAction {
    RestorePre,
    RestoreSingle,
    IntersectLive,
}

/// Mirrors Zig `planMultiBranchStateMerge`
/// (control_flow_rules.zig:205).
pub fn plan_multi_branch_state_merge(
    live_branch_count: usize,
) -> MultiBranchStateMergeAction {
    match live_branch_count {
        0 => MultiBranchStateMergeAction::RestorePre,
        1 => MultiBranchStateMergeAction::RestoreSingle,
        _ => MultiBranchStateMergeAction::IntersectLive,
    }
}

/// Mirrors Zig `intersectLiveBranchValueStates`
/// (control_flow_rules.zig:214): the state a binding has after a
/// multi-way branch, given its state at the end of each live branch.
pub fn intersect_live_branch_value_states(states: &[ValueState]) -> ValueState {
    if states.is_empty() {
        return ValueState::Active;
    }
    let first = states[0];
    if states.iter().all(|s| *s == first) {
        return first;
    }
    if states.iter().any(|s| *s == ValueState::Uninitialized) {
        return ValueState::Uninitialized;
    }
    ValueState::Consumed
}

/// Mirrors Zig `MacroParamConsumption` (control_flow_rules.zig:63).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroParamConsumption {
    Never,
    Always,
    Conditional,
}

/// `sequenceConsumption` (control_flow_rules.zig:69).
fn sequence_consumption(
    a: MacroParamConsumption,
    b: MacroParamConsumption,
) -> MacroParamConsumption {
    if a == MacroParamConsumption::Always || b == MacroParamConsumption::Always {
        return MacroParamConsumption::Always;
    }
    if a == MacroParamConsumption::Conditional || b == MacroParamConsumption::Conditional {
        return MacroParamConsumption::Conditional;
    }
    MacroParamConsumption::Never
}

/// `branchConsumption` (control_flow_rules.zig:77).
fn branch_consumption(
    a: MacroParamConsumption,
    b: MacroParamConsumption,
) -> MacroParamConsumption {
    if a == b {
        return a;
    }
    MacroParamConsumption::Conditional
}

fn consumption_of_name(name: &str, param: &str) -> MacroParamConsumption {
    if name == param {
        MacroParamConsumption::Always
    } else {
        MacroParamConsumption::Never
    }
}

fn node_consumption(node: &Node, param: &str) -> MacroParamConsumption {
    match node {
        Node::Identifier(name) => consumption_of_name(name, param),
        Node::MoveExpr(m) => {
            // Zig: a `move` of the parameter consumes it unconditionally
            // when the moved operand names the parameter.
            match &*m.expr {
                Node::Identifier(name) => consumption_of_name(name, param),
                other => node_consumption(other, param),
            }
        }
        Node::TryExpr(t) => match &*t.expr {
            Node::Identifier(name) => consumption_of_name(name, param),
            other => node_consumption(other, param),
        },
        Node::BorrowExpr(b) => node_consumption(&b.expr, param),
        Node::DerefExpr(d) => node_consumption(&d.expr, param),
        Node::FieldExpr(f) => node_consumption(&f.expr, param),
        Node::BinaryExpr(b) => sequence_consumption(
            node_consumption(&b.left, param),
            node_consumption(&b.right, param),
        ),
        Node::CallExpr(c) => {
            let mut acc = MacroParamConsumption::Never;
            for arg in &c.args {
                acc = sequence_consumption(acc, node_consumption(arg, param));
            }
            acc
        }
        Node::IndexExpr(i) => sequence_consumption(
            node_consumption(&i.target, param),
            node_consumption(&i.index, param),
        },
        Node::IfExpr(i) => {
            let cond_c = node_consumption(&i.cond, param);
            let then_c = block_consumption(&i.then_block, param);
            let else_c = match &i.else_block {
                Some(b) => block_consumption(b, param),
                None => MacroParamConsumption::Never,
            };
            sequence_consumption(cond_c, branch_consumption(then_c, else_c))
        }
        Node::WhileStmt(w) => {
            let body_c = block_consumption(&w.body, param);
            if body_c == MacroParamConsumption::Never {
                node_consumption(&w.cond, param)
            } else {
                // A loop body may run zero times: consumption inside is
                // at best conditional (control_flow_rules.zig loop arm).
                sequence_consumption(
                    node_consumption(&w.cond, param),
                    MacroParamConsumption::Conditional,
                )
            }
        }
        Node::ForStmt(f) => {
            let body_c = block_consumption(&f.body, param);
            if body_c == MacroParamConsumption::Never {
                MacroParamConsumption::Never
            } else {
                MacroParamConsumption::Conditional
            }
        }
        Node::ReturnStmt(r) => match &r.value {
            Some(v) => node_consumption(v, param),
            None => MacroParamConsumption::Never,
        },
        Node::ExprStmt(inner) => node_consumption(inner, param),
        Node::LetStmt(l) => node_consumption(&l.value, param),
        Node::ConstStmt(c) => node_consumption(&c.value, param),
        Node::AssignStmt(a) => sequence_consumption(
            node_consumption(&a.target, param),
            node_consumption(&a.value, param),
        ),
        Node::BlockStmt(b) => block_consumption(&b.body, param),
        Node::ReleaseStmt(r) => consumption_of_name(&r.var_name, param),
        _ => MacroParamConsumption::Never,
    }
}

fn block_consumption(body: &[Node], param: &str) -> MacroParamConsumption {
    let mut acc = MacroParamConsumption::Never;
    for stmt in body {
        acc = sequence_consumption(acc, node_consumption(stmt, param));
    }
    acc
}

/// Mirrors Zig `macroParamConsumption` (control_flow_rules.zig:84):
/// static analysis of a macro body deciding whether an argument bound
/// to `param` is consumed never / always / only conditionally.
pub fn macro_param_consumption(body: &[Node], param: &str) -> MacroParamConsumption {
    block_consumption(body, param)
}
