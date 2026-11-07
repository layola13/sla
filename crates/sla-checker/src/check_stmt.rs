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
                self.check_while(&w.cond, w.let_pattern.as_ref(), &w.body)
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
            Node::UsingDecl(u) => {
                // Zig checkStmt using arm (:2814): append the
                // NORMALIZED path to the CURRENT scope's list.
                let normalized =
                    crate::types_util::normalize_using_path(&u.path);
                if let Some(frame) = self.scopes.frames.last_mut() {
                    frame.using_modules.push(normalized);
                }
                Ok(())
            }
            Node::ReleaseStmt(r) => self.check_release(&r.var_name),
            Node::LetElseStmt(l) => self.check_let_else(l),
            Node::LetDestructureStmt(l) => self.check_let_destructure(l),
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
        // Slice coercion (Zig let arm only — the const arm has NO
        // slice path): a declared `Slice<T>` accepts a borrow of an
        // array of T, defines the binding, and returns WITHOUT the
        // implicit-move consumption.
        if !is_const {
            if let Some(d) = declared {
                if let Some(elem) =
                    crate::types_util::slice_element_type(d)
                {
                    if let Type::Borrow(inner) = &val_ty {
                        if let Some((arr_elem, _)) =
                            crate::types_util::array_type_of(inner)
                        {
                            if crate::types_util::types_equal(&elem, &arr_elem)
                            {
                                if !is_discard_name(name) {
                                    self.define_symbol_with_state(
                                        name,
                                        d.clone(),
                                        is_const,
                                        ValueState::Active,
                                        None,
                                    )?;
                                }
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        let expected = match declared {
            Some(d) => {
                // Dyn coercions (both let and const arms): record
                // the coercion in the side table and fall through
                // with the declared type.
                let dyn_trait = self.can_coerce_to_dyn_box(d, &val_ty).or_else(
                    || self.can_coerce_rc_new_to_dyn_rc(d, value, &val_ty),
                );
                match dyn_trait {
                    Some((is_rc, trait_name)) => {
                        let table = if is_rc {
                            &mut self.dyn_rc_coercions
                        } else {
                            &mut self.dyn_box_coercions
                        };
                        table.insert(
                            crate::types_util::node_key_of(value),
                            trait_name,
                        );
                    }
                    None => {
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
                    }
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
        // The implicit-move consumption exists only in the `let`
        // arm; the Zig const arm never consumes its initializer's
        // source (the phase-1 mirror consumed for both).
        if !is_const {
            if let Node::Identifier(src) = value {
                if !self.type_is_copy(&val_ty) && !is_borrow_like_type(&val_ty)
                {
                    self.consume_binding(src, "implicit let move")?;
                }
            }
        }
        Ok(())
    }

    /// Mirrors Zig `canCoerceToDynBox`: declared `Box<__dyn_T>`
    /// against a `Box<X>` value whose X implements T.
    /// Returns (false, trait_name) on success.
    fn can_coerce_to_dyn_box(
        &self,
        declared: &Type,
        val_ty: &Type,
    ) -> Option<(bool, String)> {
        let declared_inner = crate::types_util::box_inner_type(declared)?;
        let trait_name =
            crate::types_util::dyn_trait_name(&declared_inner)?.to_string();
        let val_inner = crate::types_util::box_inner_type(val_ty)?;
        if self.type_implements_trait(&val_inner, &trait_name) {
            Some((false, trait_name))
        } else {
            None
        }
    }

    /// Mirrors Zig `canCoerceRcNewToDynRc`: declared `Rc<__dyn_T>`
    /// against an `Rc::new(x)` call producing `Rc<X>`, X: T.
    /// Returns (true, trait_name) on success.
    fn can_coerce_rc_new_to_dyn_rc(
        &self,
        declared: &Type,
        value: &Node,
        val_ty: &Type,
    ) -> Option<(bool, String)> {
        let declared_inner = crate::types_util::rc_inner_type(declared)?;
        let trait_name =
            crate::types_util::dyn_trait_name(&declared_inner)?.to_string();
        let Node::CallExpr(call) = value else {
            return None;
        };
        if call.associated_target.as_deref() != Some("Rc")
            || call.func_name != "new"
        {
            return None;
        }
        let val_inner = crate::types_util::rc_inner_type(val_ty)?;
        if self.type_implements_trait(&val_inner, &trait_name) {
            Some((true, trait_name))
        } else {
            None
        }
    }

    /// Mirrors the `let_else_stmt` arm (type_checker.zig:2858):
    /// the value is typed, the else block is checked (in its own
    /// block scope, BEFORE the pattern bindings exist), the else
    /// block must diverge, and only then are the pattern bindings
    /// defined in the current scope — with `is_const = false`,
    /// the Zig `writable` argument's literal value (D-C15).
    fn check_let_else(
        &mut self,
        stmt: &sla_ast::stmts::LetElseStmt,
    ) -> CheckResult<()> {
        let value_ty = self.check_expr(&stmt.value)?;
        self.check_block(&stmt.else_block)?;
        if !crate::check_program::block_terminates(&stmt.else_block) {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                "let else block must diverge",
            ));
        }
        self.define_pattern_bindings(&stmt.pattern, &value_ty, "let else", false)
    }

    /// Mirrors the `let_destructure_stmt` arm
    /// (type_checker.zig:2866-2950), including the LAST unported
    /// `consumeBinding` site of the D-C7 census (:2913): after the
    /// new bindings are defined, the value's ROOT binding (through
    /// index/field projections) is consumed UNCONDITIONALLY — no
    /// Copy or borrow exemption, unlike the implicit let move
    /// (D-C12 / FB-010; venue-verified: destructuring a `(i32, i32)`
    /// tuple variable leaves it consumed).
    fn check_let_destructure(
        &mut self,
        stmt: &sla_ast::stmts::LetDestructureStmt,
    ) -> CheckResult<()> {
        let val_ty = self.check_expr(&stmt.value)?;
        if stmt.is_slice {
            let elem_ty = match crate::types_util::array_type_of(&val_ty) {
                Some((elem, _)) => elem,
                None => match crate::types_util::slice_element_type(&val_ty) {
                    Some(elem) => elem,
                    None => {
                        return Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            "slice destructuring requires array or slice value",
                        ))
                    }
                },
            };
            if stmt.rest_name.is_none() {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidArgsCount,
                    "slice destructuring requires a rest binding",
                ));
            }
            if !stmt.names.is_empty() {
                if let Some((_, arr_len)) =
                    crate::types_util::array_type_of(&val_ty)
                {
                    if arr_len < stmt.names.len() {
                        return Err(Diagnostic::new(
                            ErrorKind::InvalidArgsCount,
                            format!(
                                "slice destructuring arity mismatch: pattern={}, array={}",
                                stmt.names.len(),
                                arr_len
                            ),
                        ));
                    }
                }
            }
            for name in &stmt.names {
                if !is_discard_name(name) {
                    self.define_symbol(name, elem_ty.clone(), false)?;
                }
            }
            let slice_ty = Type::UserDefined(sla_ast::types::UserDefined {
                name: "Slice".to_string(),
                generics: vec![elem_ty.clone()],
            });
            if let Some(rest) = &stmt.rest_name {
                if !is_discard_name(rest) {
                    self.define_symbol(rest, slice_ty.clone(), false)?;
                }
            }
            if let Some(alias) = &stmt.rest_alias {
                if !is_discard_name(alias) {
                    self.define_symbol(alias, slice_ty.clone(), false)?;
                }
            }
        } else {
            let elems = match &val_ty {
                Type::Tuple(t) => t.elems.clone(),
                _ => {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        "let destructuring requires tuple value",
                    ))
                }
            };
            if stmt.names.len() != elems.len() {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidArgsCount,
                    format!(
                        "let destructuring arity mismatch: pattern={}, tuple={}",
                        stmt.names.len(),
                        elems.len()
                    ),
                ));
            }
            for (name, elem_ty) in stmt.names.iter().zip(elems.iter()) {
                if !is_discard_name(name) {
                    self.define_symbol(name, elem_ty.clone(), false)?;
                }
            }
        }
        if let Some(root) =
            crate::types_util::root_identifier_deep(&stmt.value)
        {
            let root = root.to_string();
            match self.lookup_state(&root) {
                None => {
                    return Err(Diagnostic::new(
                        ErrorKind::UndefinedVariable,
                        format!("UndefinedVariable: `{root}`"),
                    ))
                }
                Some(ValueState::Consumed) => {
                    return Err(Diagnostic::new(
                        ErrorKind::UseAfterMove,
                        format!("UseAfterMove: `{root}` was already consumed"),
                    ))
                }
                _ => {}
            }
            self.consume_binding(&root, "destructuring move")?;
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
    fn check_while(
        &mut self,
        cond: &Node,
        let_pattern: Option<&sla_ast::exprs::EnumPattern>,
        body: &[Node],
    ) -> CheckResult<()> {
        let cond_ty = self.check_expr(cond)?;
        if let_pattern.is_none() {
            let ok = matches!(
                cond_ty,
                Type::Primitive(Primitive::Boolean)
                    | Type::Primitive(Primitive::Integer)
            );
            if !ok {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    "while condition must be bool/int unless using while let",
                ));
            }
        }
        let pre = self.scopes.save_states();
        let frame = self.scopes.push_frame();
        // `while let`: the pattern bindings live in the loop scope
        // (Zig defines them in `loop_scope` before checking the
        // body), with `is_const = true` — the Zig `writable`
        // argument's literal value (D-C15).
        if let Some(pattern) = let_pattern {
            if let Err(e) =
                self.define_pattern_bindings(pattern, &cond_ty, "while let", true)
            {
                self.scopes.pop_frame();
                return Err(e);
            }
        }
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
