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
    node_key_of, node_tag, peel_borrow_pointer, type_desc, type_tag_name,
    types_equal, value_assignable_to,
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
            Node::AssignStmt(a) => {
                self.arena_check_assign(&a.target, &a.value)?;
                self.check_assign(&a.target, &a.value)
            }
            Node::BlockStmt(b) => self.check_block(&b.body),
            Node::ArenaStmt(b) => self.check_arena_block(&b.body),
            Node::GoStmt(g) => {
                // Statement position: the handle is discarded.
                self.check_go_stmt(g).map(|_| ())
            }
            Node::DeferStmt(d) => self.check_defer_stmt(d),
            Node::SelectStmt(s) => {
                self.check_select_stmt(s, crate::types_util::node_key_of(node))
            }
            Node::ExprStmt(inner) => {
                let ty = self.check_expr(inner)?;
                // R-6 (F1): a bare statement that discards a
                // fallible call's Result is FallibleResultDiscarded
                // (A_min must-consume).
                self.check_fallible_stmt_consumption(inner, &ty)?;
                Ok(())
            }
            Node::ReturnStmt(r) => {
                // Staged `checking_return_value`: escapes through a
                // return value never stay dead across a loop
                // back-edge, so consumption inside the return value
                // is exempt from the LoopConditionalConsume ban
                // (REG-LOOP-CONSUME positive). Held on the loop
                // context so nested statement checks (ifs, struct
                // literal fields, call args) all see it.
                let saved_ret = self.loop_ctx.last().map(|l| l.in_return_value);
                if let Some(top) = self.loop_ctx.last_mut() {
                    top.in_return_value = true;
                }
                let result = self.check_return(node, r.value.as_deref());
                if let Some(top) = self.loop_ctx.last_mut() {
                    top.in_return_value = saved_ret.unwrap_or(false);
                }
                result
            }
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
        // DEF-RUST-FITER-HELDCHAIN-RESIDUAL: compute the value's
        // chain stages BEFORE defining the binding (see
        // `chain_stages_for_let`): the value's identifiers must
        // resolve to the pre-let bindings — a shadowing let's value
        // refers to the outer binding, mirroring the codegen,
        // which resolves the RHS before pushing the new alias.
        // Const lets never hold a chain (a chain is a runtime
        // view, not a compile-time value).
        let chain_stages = if is_const {
            None
        } else {
            self.chain_stages_for_let(value)
        };
        // DEF-RUST-STRPTR-CHECKER: refineStringLiteralForSlot at
        // the let / fn-const sites (Zig :7486 / :7620; see helper).
        let slot_ty = declared.cloned().unwrap_or_else(|| val_ty.clone());
        let val_ty = self.refine_string_literal_for_slot(value, val_ty, &slot_ty);
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
        // Borrow-source bookkeeping (issue013 guard consults this).
        let borrow_source = if is_borrow_like_type(&expected) {
            crate::check_stmt_util::root_identifier(value).map(|s| s.to_string())
        } else {
            None
        };
        if !is_discard_name(name) {
            let binding_id = self.define_symbol_with_state(
                name,
                expected.clone(),
                is_const,
                ValueState::Active,
                borrow_source,
            )?;
            // DEF-RUST-FITER-HELDCHAIN-RESIDUAL: record a held
            // iterator-chain binding (mirrors codegen
            // gen_let_stmt, which inserts at the let site).
            // Reassignment never clears the entry — the codegen
            // never removes from `iter_chain_bindings` either
            // (insert at let, clear per function), so the mirror
            // is exact; a reassigned chain binding is the
            // codegen's shape, not the checker's, to dispute.
            if let Some(stages) = chain_stages {
                self.chain_bindings.insert(binding_id, stages);
            }
            // R-3: arena domain gates for the new binding.
            self.arena_on_let(name, &expected, value)?;
            // R-3: borrow-origin registration for view bindings.
            self.origin_on_bind(name, value, &expected)?;
        }
        // Implicit-move consumption exists only in the `let` arm.
        if !is_const {
            if let Node::Identifier(src) = value {
                if !self.type_is_copy(&val_ty) && !is_borrow_like_type(&val_ty)
                {
                    // Zig let-arm guard (Scope.lookup, type_checker.zig ~7596):
                    // only consume when the source is a scope binding. Fn items
                    // live in `funcs`, not in scope -- consuming one is a bogus
                    // UndefinedVariable (R255-TRIAGE-FORM2).
                    if self.scopes.lookup(src).is_some() {
                        // R-6 (F3): BorrowConflict gate.
                        self.check_let_move_root_free(src, name)?;
                        // R-3: moving an owner invalidates its views.
                        self.origin_on_move(src);
                        self.consume_binding(src, "implicit let move")?;
                    }
                }
            }
        }
        // R-6 (F3): register the field borrow on the new binding.
        if !is_const && !is_discard_name(name) {
            self.register_borrow_from_value(name, value, &expected);
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
            // FB-010: mirror the plain-let implicit-move exemption
            // (Zig type_checker.zig let_destructure_stmt arm). A Copy
            // or borrow-like root binding is not consumed by
            // destructuring out of it; the codegen destructure arm
            // already skips the root release on the same caliber
            // (copy_root), so the two sides now agree.
            let root_exempt = self
                .lookup_ty(&root)
                .map(|t| self.type_is_copy(&t) || is_borrow_like_type(&t))
                .unwrap_or(false);
            if !root_exempt {
                self.consume_binding(&root, "destructuring move")?;
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
            // DEF-RUST-CONST-NOT-ENFORCED (task 74): a `const`
            // binding is immutable — assigning to it is a loud
            // error, mirroring Zig type_checker.zig:8125 ("let
            // bindings can be reassigned and indexed; const
            // bindings cannot"). `check_let` records const-ness
            // in the scope's `is_const` slot (top-level consts
            // via check_program.rs:640, local consts via
            // ConstStmt -> check_let); `let` bindings and
            // parameters stay mutable (`is_const = false`).
            if self.scopes.lookup(name).map(|b| b.is_const).unwrap_or(false) {
                return Err(Diagnostic::new(
                    ErrorKind::ConstReassign,
                    format!("ConstReassign: cannot assign to const binding `{name}`"),
                ));
            }
            // R-6 (F3): reassigning a root with a live field borrow
            // is a BorrowConflict; otherwise the assignment drops
            // the target's own registered borrows.
            self.check_assign_root_free(name)?;
            let val_ty = self.check_expr(value)?;
            // DEF-RUST-STRPTR-CHECKER: assign-site refinement (Zig :7693).
            let val_ty = self.refine_string_literal_for_slot(value, val_ty, &target_ty);
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
            // R-3: borrow-origin merge/rebind gate.
            self.origin_on_assign(name, value, &val_ty)?;
            if let Node::Identifier(src) = value {
                if !self.type_is_copy(&val_ty) && !is_borrow_like_type(&val_ty) {
                    self.consume_binding(src, "assignment move")?;
                }
            }
            // Assignment (re)initializes the slot and clears any
            // consumption marks (R-6).
            self.set_state(name, ValueState::Active);
            self.clear_consumed_marks(name);
            // R-6 (F3): assigning a borrowed place re-registers
            // the borrow against the target (Zig gates this on the
            // target type being borrow-like).
            self.register_borrow_from_value(name, value, &target_ty);
            return Ok(());
        }
        // Field / index / deref targets: a direct write to a place
        // borrowed live is a BorrowConflict (R-6 F3); deref targets
        // write THROUGH a borrow and are exempt (the gate's
        // place_of_expr yields no place for them at the root).
        if !matches!(target, Node::DerefExpr(_)) {
            self.check_assign_place_free(target)?;
        }
        // DEF-RUST-CONST-NOT-ENFORCED (task 74): a field/index
        // write whose place roots at a `const` binding is the
        // same violation one level down. Deref targets write
        // THROUGH the binding and stay exempt, like the
        // BorrowConflict gate above.
        if !matches!(target, Node::DerefExpr(_)) {
            if let Some(place) = crate::check_field_borrow::place_of_expr(target) {
                if self
                    .scopes
                    .lookup(&place.root)
                    .map(|b| b.is_const)
                    .unwrap_or(false)
                {
                    return Err(Diagnostic::new(
                        ErrorKind::ConstReassign,
                        format!(
                            "ConstReassign: cannot assign to field of const binding `{}`",
                            place.root
                        ),
                    ));
                }
            }
        }
        let target_ty = self.check_expr(target)?;
        let val_ty = self.check_expr(value)?;
        // DEF-RUST-STRPTR-CHECKER: assign-site refinement (Zig :7693).
        let val_ty = self.refine_string_literal_for_slot(value, val_ty, &target_ty);
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
        // R-3: arena escape gate on the return value (route 1).
        self.arena_check_return(value)?;
        // R-3: borrow-origin return gate.
        if let Some(func) = self.current_func.clone() {
            self.origin_check_return(&func, value)?;
        }
        if let Some(v) = value {
            let val_ty = self.check_expr(v)?;
            // Element-borrow return gate: a borrow whose place
            // roots at a function-local owned binding must not
            // escape through the return — the root is dropped
            // at exit (corpus test_error_growvec_ref_escape).
            let root_ty = self.borrow_place_of_value(v).and_then(|place| {
                self.scopes.lookup(&place.root).map(|b| b.ty.clone())
            });
            self.check_return_borrow_escape(v, &val_ty, root_ty.as_ref())?;
            // DEF-RUST-STRPTR-CHECKER: return-site refinement (Zig :7807).
            let val_ty = self.refine_string_literal_for_slot(v, val_ty, &ret_ty);
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
                    // issue013 (type_checker.zig `issue013SkipApplies`,
                    // applied in the return collection): a borrow
                    // binding that another still-active binding was
                    // aliased from is settled by the alias's own
                    // release — codegen emits a bare register move
                    // for the alias let, so releasing the source too
                    // retires a consumed register (UnknownRegister).
                    // The raw_ptr narrowing is folded in: borrow-like
                    // here is Borrow | RawPtr, and a raw_ptr alias is
                    // a distinct register copy, so only Borrow skips.
                    if matches!(b.ty, Type::Borrow(_))
                        && self.borrow_source_aliased_by_other_active(&b.name)
                    {
                        continue;
                    }
                    names.push(b.name.clone());
                }
            }
            if !names.is_empty() {
                self.cleanups.insert(node_key_of(node), names);
            }
        } else if matches!(ret_ty, Type::Primitive(Primitive::VoidType)) {
            // M2(a): a bare `return;` is a function exit too — record
            // the same exit sweep as the valued path (innermost frame
            // down to the function root, parameters included; there
            // is no result expression, so nothing escapes). Without
            // it gen_return_stmt emits no cleanups at this exit and
            // still-live parameters leak (Zig's return path collects
            // them the same way).
            let mut names = Vec::new();
            let mut idx = self.scopes.frames.len();
            while idx > self.fn_root_depth {
                idx -= 1;
                for b in &self.scopes.frames[idx].bindings {
                    if b.state != ValueState::Active || is_internal_symbol(&b.name) {
                        continue;
                    }
                    // issue013 skip, same as the valued path above
                    // (Zig's return collection applies it uniformly).
                    if matches!(b.ty, Type::Borrow(_))
                        && self.borrow_source_aliased_by_other_active(&b.name)
                    {
                        continue;
                    }
                    names.push(b.name.clone());
                }
            }
            if !names.is_empty() {
                self.cleanups.insert(node_key_of(node), names);
            }
        } else {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                "TypeMismatch in return: expected a value",
            ));
        }
        Ok(())
    }

    /// `for`: range form (`start..end`, loop variable takes the bound
    /// type) and collection form (loop variable takes the element
    /// type of an array, or of a type implementing the iterable
    /// protocol). Body checked exactly once.
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
            // Zig for_stmt collection arm (type_checker.zig:8017):
            // an array iterates its element type; any other value
            // must implement the iterable protocol (`iter_len` /
            // `iter_at`, see protocol_iterable_element_type) and
            // iterates `iter_at`'s return type. Anything else is a
            // TypeMismatch naming the value's tag.
            match peel_borrow_pointer(&coll_ty) {
                Type::Array(a) => (*a.elem).clone(),
                _ => match self.protocol_iterable_element_type(&coll_ty) {
                    Some(elem) => elem,
                    None => {
                        return Err(Diagnostic::new(
                            ErrorKind::TypeMismatch,
                            format!(
                                "for iterable value must be array or implement Iterable: start tag={}",
                                type_tag_name(&coll_ty)
                            ),
                        ))
                    }
                },
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
            in_return_value: false,
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
            in_return_value: false,
        });
        let result = self.check_block(body);
        self.loop_ctx.pop();
        self.scopes.pop_frame();
        self.scopes.restore_uninitialized_from_saved(&pre);
        result
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
