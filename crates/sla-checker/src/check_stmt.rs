//! Statement checking, mirroring the `checkStmt` arms of
//! `type_checker.zig` (wrapper at 2791, arms from 2817).
//!
//! The ownership-relevant arms are `let` / assignment (implicit moves
//! of bare identifiers), `release` (explicit consumption), and
//! `return` (exit cleanup collection). Loop arms share the
//! save/check-once/restore-uninitialized pattern (D-C4).

use crate::checker::{Checker, LoopCtx};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::rules::{plan_function_exit_cleanup, FunctionExitCleanupAction, ValueState};
use crate::types_util::{
    is_borrow_like_type, is_discard_name, is_internal_symbol, is_primitive_scalar,
    node_key_of, node_tag, type_desc, types_equal, value_assignable_to,
};
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type};

impl Checker {
    pub fn check_stmt(&mut self, node: &Node) -> CheckResult<()> {
        match node {
            Node::LetStmt(l) => self.check_let(&l.name, l.ty.as_deref(), &l.value, false),
            Node::ConstStmt(c) => self.check_let(&c.name, c.ty.as_deref(), &c.value, true),
            Node::VarStmt(v) => {
                if !is_primitive_scalar(&v.ty) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        "var Phase 1 supports scalar primitive slots only",
                    ));
                }
                if !is_discard_name(&v.name) {
                    self.define_symbol_with_state(
                        &v.name,
                        (*v.ty).clone(),
                        false,
                        ValueState::Uninitialized,
                        None,
                    )?;
                }
                Ok(())
            }
            Node::AssignStmt(a) => self.check_assign(&a.target, &a.value),
            Node::BlockStmt(b) => self.check_block(&b.body),
            Node::ExprStmt(inner) => {
                self.check_expr(inner)?;
                Ok(())
            }
            Node::ReturnStmt(r) => self.check_return(node, r.value.as_deref()),
            Node::ForStmt(f) => {
                let end = f.end.as_deref();
                self.check_for(&f.var_name, &f.start, end, &f.body)
            }
            Node::WhileStmt(w) => {
                if w.let_pattern.is_some() {
                    return Err(Diagnostic::out_of_slice("while let_pattern"));
                }
                self.check_while(&w.cond, &w.body)
            }
            Node::BreakStmt(_) => {
                let names = self.collect_break_cleanups();
                if !names.is_empty() {
                    self.cleanups.insert(node_key_of(node), names);
                }
                Ok(())
            }
            Node::ContinueStmt(_) => {
                let names = self.collect_break_cleanups();
                if !names.is_empty() {
                    self.cleanups.insert(node_key_of(node), names);
                }
                Ok(())
            }
            Node::ReleaseStmt(r) => self.check_release(&r.var_name),
            Node::LetElseStmt(_) | Node::LetDestructureStmt(_) => {
                Err(Diagnostic::out_of_slice(node_tag(node)))
            }
            _ => Err(Diagnostic::new(
                ErrorKind::CompileError,
                format!("checkStmt failed at node tag {}", node_tag(node)),
            )),
        }
    }

    /// `let` / `const`: type the value, check assignability against
    /// the declared type, define the binding, then apply the implicit
    /// let move when the value is a bare non-Copy identifier.
    fn check_let(
        &mut self,
        name: &str,
        declared: Option<&Type>,
        value: &Node,
        is_const: bool,
    ) -> CheckResult<()> {
        let val_ty = self.check_expr(value)?;
        let expected = match declared {
            Some(d) => {
                if !value_assignable_to(d, &val_ty) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "TypeMismatch: expected {}, actual {}",
                            type_desc(d),
                            type_desc(&val_ty)
                        ),
                    ));
                }
                d.clone()
            }
            None => val_ty.clone(),
        };
        // Borrow-source bookkeeping: a borrow-like value taken from a
        // bare identifier records its origin binding (issue013 guard
        // at block exit consults this).
        let borrow_source = if is_borrow_like_type(&expected) {
            root_identifier(value).map(|s| s.to_string())
        } else {
            None
        };
        if !is_discard_name(name) {
            self.define_symbol_with_state(
                name,
                expected.clone(),
                is_const,
                ValueState::Active,
                borrow_source,
            )?;
        }
        if let Node::Identifier(src) = value {
            if !self.type_is_copy(&val_ty) && !is_borrow_like_type(&val_ty) {
                self.consume_binding(src, "implicit let move")?;
            }
        }
        Ok(())
    }

    fn check_assign(&mut self, target: &Node, value: &Node) -> CheckResult<()> {
        if let Node::Identifier(name) = target {
            let target_ty = match self.lookup_ty(name) {
                Some(t) => t,
                None => {
                    return Err(Diagnostic::new(
                        ErrorKind::UndefinedVariable,
                        format!("UndefinedVariable: `{name}`"),
                    ))
                }
            };
            let val_ty = self.check_expr(value)?;
            if !value_assignable_to(&target_ty, &val_ty) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "TypeMismatch: expected {}, actual {}",
                        type_desc(&target_ty),
                        type_desc(&val_ty)
                    ),
                ));
            }
            if let Node::Identifier(src) = value {
                if !self.type_is_copy(&val_ty) && !is_borrow_like_type(&val_ty) {
                    self.consume_binding(src, "assignment move")?;
                }
            }
            // Assignment (re)initializes the slot.
            self.set_state(name, ValueState::Active);
            return Ok(());
        }
        // Field / index / deref targets: type both sides and compare.
        let target_ty = self.check_expr(target)?;
        let val_ty = self.check_expr(value)?;
        if !value_assignable_to(&target_ty, &val_ty) {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "TypeMismatch: expected {}, actual {}",
                    type_desc(&target_ty),
                    type_desc(&val_ty)
                ),
            ));
        }
        Ok(())
    }

    fn check_return(&mut self, node: &Node, value: Option<&Node>) -> CheckResult<()> {
        let ret_ty = match &self.current_ret_ty {
            Some(t) => t.clone(),
            None => {
                return Err(Diagnostic::new(
                    ErrorKind::CompileError,
                    "return statement outside of a function",
                ))
            }
        };
        if let Some(v) = value {
            let val_ty = self.check_expr(v)?;
            let ret_is_void = matches!(ret_ty, Type::Primitive(Primitive::VoidType));
            if ret_is_void && !matches!(val_ty, Type::Primitive(Primitive::VoidType)) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    "TypeMismatch in return: expected void_type, actual value",
                ));
            }
            if !ret_is_void && !value_assignable_to(&ret_ty, &val_ty) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "TypeMismatch in return: expected {}, actual {}",
                        type_desc(&ret_ty),
                        type_desc(&val_ty)
                    ),
                ));
            }
            // Exit cleanups: every still-active binding from the
            // innermost frame down to the function root, except the
            // binding whose value escapes through the result.
            let mut names = Vec::new();
            let mut idx = self.scopes.frames.len();
            while idx > self.fn_root_depth {
                idx -= 1;
                for b in &self.scopes.frames[idx].bindings {
                    if b.state != ValueState::Active || is_internal_symbol(&b.name) {
                        continue;
                    }
                    if plan_function_exit_cleanup(&b.name, Some(v))
                        == FunctionExitCleanupAction::TransferResult
                    {
                        continue;
                    }
                    names.push(b.name.clone());
                }
            }
            if !names.is_empty() {
                self.cleanups.insert(node_key_of(node), names);
            }
        } else if !matches!(ret_ty, Type::Primitive(Primitive::VoidType)) {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                "TypeMismatch in return: expected a value",
            ));
        }
        Ok(())
    }

    /// `for`: range form (`start..end`, loop variable takes the bound
    /// type) and collection form (loop variable takes the element
    /// type of an array / Vec-like). Body checked exactly once.
    fn check_for(
        &mut self,
        var_name: &str,
        start: &Node,
        end: Option<&Node>,
        body: &[Node],
    ) -> CheckResult<()> {
        let var_ty = if let Some(end_node) = end {
            let start_ty = self.check_expr(start)?;
            let end_ty = self.check_expr(end_node)?;
            if !types_equal(&start_ty, &end_ty) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    "for range bounds have mismatched types",
                ));
            }
            start_ty
        } else {
            let coll_ty = self.check_expr(start)?;
            match coll_ty {
                Type::Array(a) => (*a.elem).clone(),
                Type::UserDefined(u) if !u.generics.is_empty() => u.generics[0].clone(),
                _ => {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        "for collection is not iterable in this slice",
                    ))
                }
            }
        };
        let pre = self.scopes.save_states();
        let frame = self.scopes.push_frame();
        self.define_symbol_with_state(
            var_name,
            var_ty,
            true,
            ValueState::Active,
            None,
        )?;
        self.loop_ctx.push(LoopCtx {
            frame_index: frame,
            body_terminates: crate::check_program::block_terminates(body),
        });
        let result = self.check_block(body);
        self.loop_ctx.pop();
        self.scopes.pop_frame();
        self.scopes.restore_uninitialized_from_saved(&pre);
        result
    }

    /// `while`: condition must be boolean or the LEGACY `integer`
    /// primitive only — sized integers (i32, i64, ...) are rejected
    /// by the Zig arm (type_checker.zig ~3078-3085), which accepts
    /// exactly `.boolean, .integer`. (On the sized-int path Zig
    /// returns a bare TypeMismatch with NO message; see D-C9.)
    /// Body checked exactly once — there is NO dataflow fixpoint; the
    /// LoopConditionalConsume ban inside `consume_binding` is the
    /// compensating approximation (D-C4). Afterwards only
    /// previously-uninitialized bindings roll back.
    fn check_while(&mut self, cond: &Node, body: &[Node]) -> CheckResult<()> {
        let cond_ty = self.check_expr(cond)?;
        let ok = matches!(
            cond_ty,
            Type::Primitive(Primitive::Boolean) | Type::Primitive(Primitive::Integer)
        );
        if !ok {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                "while condition must be bool/int unless using while let",
            ));
        }
        let pre = self.scopes.save_states();
        let frame = self.scopes.push_frame();
        self.loop_ctx.push(LoopCtx {
            frame_index: frame,
            body_terminates: crate::check_program::block_terminates(body),
        });
        let result = self.check_block(body);
        self.loop_ctx.pop();
        self.scopes.pop_frame();
        self.scopes.restore_uninitialized_from_saved(&pre);
        result
    }

    /// `release x` (the `!x` statement): explicit consumption with
    /// state pre-checks, mirroring the release arm.
    fn check_release(&mut self, name: &str) -> CheckResult<()> {
        match self.lookup_state(name) {
            None => Err(Diagnostic::new(
                ErrorKind::UndefinedVariable,
                format!("UndefinedVariable: `{name}`"),
            )),
            Some(ValueState::Consumed) => Err(Diagnostic::new(
                ErrorKind::UseAfterMove,
                format!("UseAfterMove: `{name}` was already consumed"),
            )),
            Some(ValueState::Uninitialized) => Err(Diagnostic::new(
                ErrorKind::UseBeforeInit,
                format!("UseBeforeInit: `{name}` is not initialized"),
            )),
            Some(ValueState::Active) => self.consume_binding(name, "explicit release"),
        }
    }

    /// Cleanup names for break/continue: active bindings from the
    /// innermost frame down to (and including) the loop frame.
    fn collect_break_cleanups(&self) -> Vec<String> {
        let limit = self
            .loop_ctx
            .last()
            .map(|l| l.frame_index)
            .unwrap_or(self.fn_root_depth);
        let mut names = Vec::new();
        let mut idx = self.scopes.frames.len();
        while idx > limit {
            idx -= 1;
            for b in &self.scopes.frames[idx].bindings {
                if b.state == ValueState::Active && !is_internal_symbol(&b.name) {
                    names.push(b.name.clone());
                }
            }
        }
        names
    }
}

/// Root identifier of an expression through borrow/move wrappers.
fn root_identifier(node: &Node) -> Option<&str> {
    match node {
        Node::Identifier(name) => Some(name.as_str()),
        Node::BorrowExpr(b) => root_identifier(&b.expr),
        Node::MoveExpr(m) => root_identifier(&m.expr),
        _ => None,
    }
}
