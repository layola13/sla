//! The `if` expression: branch checking and post-branch state merge,
//! mirroring the plain `if_expr` arm of `type_checker.zig`
//! (~5395-5480) and `planBranchStateMerge` of control_flow_rules.
//!
//! THE critical porting point (D-C1): Zig snapshots are flat maps
//! keyed by NAME — `saveScopeStates` writes inner scopes first and
//! outer scopes overwrite the same key, so the snapshot of a shadowed
//! name holds the OUTER binding's state, and the merge reads that one
//! state back for both bindings. Here snapshots are keyed by
//! `BindingId` (scope.rs), so each binding merges independently.
//! The merge RULE itself is unchanged: differing end states collapse
//! to `uninitialized` if either side is uninitialized, else to
//! `consumed`, and a binding left active on exactly one side earns a
//! phi cleanup on that side's last node.

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::{
    plan_branch_state_merge, BranchStateMergeAction, ValueState,
};
use crate::scope::StateSnapshot;
use crate::types_util::{node_key_of, type_desc, types_equal, void_ty};
use sla_ast::exprs::IfExpr;
use sla_ast::node::Node;
use sla_ast::types::Type;

impl Checker {
    /// Check a block and return its tail-expression type, if any.
    pub fn check_block_value(&mut self, body: &[Node]) -> CheckResult<Option<Type>> {
        self.check_block(body)?;
        Ok(crate::check_program::block_tail_type(self, body))
    }

    pub fn check_if_expr(&mut self, expr: &IfExpr) -> CheckResult<Type> {
        if expr.let_chain.is_some() {
            return self.check_if_let(expr);
        }
        let cond_ty = self.check_expr(&expr.cond)?;
        if !matches!(cond_ty, Type::Primitive(sla_ast::types::Primitive::Boolean)) {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "if condition must be boolean, actual {}",
                    type_desc(&cond_ty)
                ),
            ));
        }

        let pre = self.scopes.save_states();
        let then_ty = self.check_block_value(&expr.then_block)?;
        let then_states = self.scopes.save_states();
        let then_terminates = crate::check_program::block_terminates(&expr.then_block);
        self.scopes.restore_states(&pre);

        let (else_ty, else_states, else_terminates) = match &expr.else_block {
            Some(else_block) => {
                let ty = self.check_block_value(else_block)?;
                let states = self.scopes.save_states();
                let term = crate::check_program::block_terminates(else_block);
                (ty, states, term)
            }
            None => (None, pre.clone(), false),
        };

        match plan_branch_state_merge(then_terminates, else_terminates) {
            BranchStateMergeAction::RestorePre => self.scopes.restore_states(&pre),
            BranchStateMergeAction::RestoreThen => {
                self.scopes.restore_states(&then_states)
            }
            BranchStateMergeAction::RestoreElse => {
                self.scopes.restore_states(&else_states)
            }
            BranchStateMergeAction::KeepCurrent => {
                self.merge_branch_states(
                    &then_states,
                    &else_states,
                    expr.then_block.last(),
                    expr.else_block.as_ref().and_then(|b| b.last()),
                );
            }
        }

        match (then_ty, else_ty) {
            (Some(t), Some(e)) => {
                if types_equal(&t, &e) {
                    Ok(t)
                } else {
                    Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "if branches have mismatched types: {} vs {}",
                            type_desc(&t),
                            type_desc(&e)
                        ),
                    ))
                }
            }
            _ => Ok(void_ty()),
        }
    }

    /// The if-let chain arm (type_checker.zig:5252-5340). The chain
    /// values are checked in a fresh pattern frame (Zig's
    /// `chain_scope`), each pattern's bindings defined there in
    /// order — later conditions see earlier bindings — and the
    /// then block is checked under that frame. The else block and
    /// the state merge run on the OUTER scope chain, exactly as in
    /// Zig, whose `saveScopeStates(scope)` after the then block
    /// never includes the pattern bindings. The return type Zig
    /// reads back through `return_ty_sentinel` with a `.?` unwrap
    /// is `current_ret_ty` here (D-C5); no unwrap path exists.
    fn check_if_let(&mut self, expr: &IfExpr) -> CheckResult<Type> {
        let chain = expr.let_chain.as_ref().expect("let_chain checked");
        let pre = self.scopes.save_states();

        self.scopes.push_frame();
        let mut chain_result = Ok(());
        for cond in chain {
            let value_ty = match self.check_expr(&cond.value) {
                Ok(t) => t,
                Err(e) => {
                    chain_result = Err(e);
                    break;
                }
            };
            if let Err(e) =
                self.define_pattern_bindings(&cond.pattern, &value_ty, "if let", true)
            {
                chain_result = Err(e);
                break;
            }
        }
        let then_ty = if chain_result.is_ok() {
            match self.check_block_value(&expr.then_block) {
                Ok(t) => t,
                Err(e) => {
                    chain_result = Err(e);
                    None
                }
            }
        } else {
            None
        };
        let then_states = self.scopes.save_states();
        let then_terminates = crate::check_program::block_terminates(&expr.then_block);
        self.scopes.pop_frame();
        self.scopes.restore_states(&pre);
        chain_result?;

        let (else_ty, else_terminates) = match &expr.else_block {
            Some(else_block) => {
                let ty = self.check_block_value(else_block)?;
                let term = crate::check_program::block_terminates(else_block);
                (ty, term)
            }
            None => (None, false),
        };
        let else_states = self.scopes.save_states();

        match plan_branch_state_merge(then_terminates, else_terminates) {
            BranchStateMergeAction::RestorePre => self.scopes.restore_states(&pre),
            BranchStateMergeAction::RestoreThen => {
                self.scopes.restore_states(&then_states)
            }
            BranchStateMergeAction::RestoreElse => {}
            BranchStateMergeAction::KeepCurrent => {
                self.merge_branch_states(
                    &then_states,
                    &else_states,
                    expr.then_block.last(),
                    expr.else_block.as_ref().and_then(|b| b.last()),
                );
            }
        }

        match (then_ty, else_ty) {
            (Some(t), Some(e)) => {
                if types_equal(&t, &e) {
                    Ok(t)
                } else {
                    Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        "if expression branch type mismatch",
                    ))
                }
            }
            _ => Ok(void_ty()),
        }
    }

    /// Mirrors Zig `mergeMultiBranchScopeStates`
    /// (type_checker.zig:2738) with identity-keyed snapshots: the
    /// live-branch count selects restore-pre / restore-single /
    /// intersect-live via `plan_multi_branch_state_merge`; the
    /// intersect case folds each binding's per-branch end states
    /// with `intersect_live_branch_value_states` and records phi
    /// cleanups on the last node of every live branch where the
    /// binding stayed active. Used by `switch` and `match`.
    pub fn merge_multi_branch_states(
        &mut self,
        pre: &StateSnapshot,
        live_states: &[StateSnapshot],
        live_bodies: &[&[Node]],
    ) {
        use crate::rules::{intersect_live_branch_value_states, plan_multi_branch_state_merge, MultiBranchStateMergeAction};
        match plan_multi_branch_state_merge(live_states.len()) {
            MultiBranchStateMergeAction::RestorePre => {
                self.scopes.restore_states(pre)
            }
            MultiBranchStateMergeAction::RestoreSingle => {
                self.scopes.restore_states(&live_states[0])
            }
            MultiBranchStateMergeAction::IntersectLive => {
                let mut updates: Vec<(crate::scope::BindingId, ValueState)> =
                    Vec::new();
                let mut phi: Vec<(usize, String)> = Vec::new();
                for frame in &self.scopes.frames {
                    for b in &frame.bindings {
                        if crate::types_util::is_internal_symbol(&b.name) {
                            continue;
                        }
                        let per_branch: Vec<ValueState> = live_states
                            .iter()
                            .map(|s| {
                                s.get(&b.id).copied().unwrap_or(ValueState::Active)
                            })
                            .collect();
                        let merged =
                            intersect_live_branch_value_states(&per_branch);
                        updates.push((b.id, merged));
                        if merged == ValueState::Consumed {
                            for (state, body) in
                                per_branch.iter().zip(live_bodies.iter())
                            {
                                if *state == ValueState::Active {
                                    if let Some(last) = body.last() {
                                        phi.push((
                                            node_key_of(last),
                                            b.name.clone(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                for (id, state) in updates {
                    self.set_state_by_id(id, state);
                }
                for (key, name) in phi {
                    self.phi_cleanups.entry(key).or_default().push(name);
                }
            }
        }
    }

    /// The `keep_current` merge: for every visible binding, combine
    /// its then-end and else-end states; record phi cleanups for
    /// bindings consumed on exactly one side.
    fn merge_branch_states(
        &mut self,
        then_states: &StateSnapshot,
        else_states: &StateSnapshot,
        then_last: Option<&Node>,
        else_last: Option<&Node>,
    ) {
        // Collect first (immutable borrow), then apply.
        let mut updates: Vec<(crate::scope::BindingId, ValueState)> = Vec::new();
        let mut phi_then: Vec<String> = Vec::new();
        let mut phi_else: Vec<String> = Vec::new();
        for frame in &self.scopes.frames {
            for b in &frame.bindings {
                let then_s = then_states
                    .get(&b.id)
                    .copied()
                    .unwrap_or(ValueState::Active);
                let else_s = else_states
                    .get(&b.id)
                    .copied()
                    .unwrap_or(ValueState::Active);
                let merged = if then_s == else_s {
                    then_s
                } else if then_s == ValueState::Uninitialized
                    || else_s == ValueState::Uninitialized
                {
                    ValueState::Uninitialized
                } else {
                    ValueState::Consumed
                };
                if merged == ValueState::Consumed {
                    if then_s == ValueState::Active {
                        phi_then.push(b.name.clone());
                    }
                    if else_s == ValueState::Active {
                        phi_else.push(b.name.clone());
                    }
                }
                updates.push((b.id, merged));
            }
        }
        for (id, state) in updates {
            self.set_state_by_id(id, state);
        }
        if !phi_then.is_empty() {
            if let Some(last) = then_last {
                self.phi_cleanups
                    .entry(node_key_of(last))
                    .or_default()
                    .extend(phi_then);
            }
        }
        if !phi_else.is_empty() {
            if let Some(last) = else_last {
                self.phi_cleanups
                    .entry(node_key_of(last))
                    .or_default()
                    .extend(phi_else);
            }
        }
    }
}
