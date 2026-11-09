//! B4 (continued) — statement generation for the driver
//! subset: `gen_block` / `gen_stmt` / `gen_let` / `gen_while` /
//! `gen_loop_jump`, split from `driver_gen.rs` solely for the
//! 700-line file discipline (S2' step 0, behaviour-preserving
//! verbatim move); the two files are one decomposition unit (B4).

use sla_ast::{Literal, Node, Type, WhileStmt};

use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::release_drop::NoDropFacts;
use crate::release_emit::{emit_branch_release, NoMacroFragments};
use crate::release_locals::DropEngine;
use crate::types::Local;

impl<'c> Driver<'c> {
    // -- statements -------------------------------------------------
    /// Zig `genBlock` (lines 6764–6783): statements in order, stop
    /// after a terminator.
    pub(crate) fn gen_block(&mut self, body: &[Node]) -> Result<()> {
        for stmt in body {
            self.gen_stmt(stmt)?;
            if self.b().last_is_terminator() {
                break;
            }
        }
        Ok(())
    }

    /// Zig `genScopedBlock`: a block statement scopes its locals.
    pub(crate) fn gen_scoped_block(&mut self, body: &[Node]) -> Result<()> {
        let saved = self.state.frame.locals.len();
        let result = self.gen_block(body);
        self.state.frame.pop_locals_to(saved);
        result
    }

    /// Zig `genStmt` (lines 10142–10215), subset arms. Every other
    /// arm is a loud, group-attributed stop.
    pub(crate) fn gen_stmt(&mut self, stmt: &Node) -> Result<()> {
        match stmt {
            Node::LetStmt(l) => self.gen_let(&l.name, l.ty.as_deref(), &l.value),
            Node::ConstStmt(c) => self.gen_let(&c.name, c.ty.as_deref(), &c.value),
            Node::VarStmt(v) => {
                // Zig `genStmt` var arm (10145-10149): a fresh tmp
                // register, a stack alloc of the declared type's
                // size, and a stack local binding the name.
                let dst = self.b().new_tmp();
                crate::release_prim::emit_stack_alloc(&mut self.state, dst, crate::layout::type_size(&v.ty));
                crate::locals::push_stack_local(&mut self.state, &v.name, dst, &v.ty)
            }
            Node::AssignStmt(a) => {
                // Zig `genStmt` assign arm (10150) routes to
                // `genAssign` (10335): deref / field / index
                // targets are ported in `gen_assign.rs`;
                // identifier targets keep the path below; any
                // other target kind keeps the loud stop.
                match &*a.target {
                    Node::DerefExpr(_) | Node::FieldExpr(_) | Node::IndexExpr(_) => {
                        return self.gen_assign(a);
                    }
                    Node::Identifier(_) => {}
                    _ => {
                        return Err(unsupported(
                            "assign to non-identifier target",
                            "G gen_assign",
                            "10150",
                        ))
                    }
                }
                let name = match &*a.target {
                    Node::Identifier(name) => name.clone(),
                    _ => unreachable!("target kind checked above"),
                };
                let src = self.gen_expr(&a.value)?;
                // Zig `assignToIdentifier` (8837-8846): a target
                // homed in a stack slot is stored through the slot,
                // not assigned by register.
                if let Some(slot) = crate::captures::stack_local(&self.state, &name) {
                    let ty = slot.stack_ty.clone().ok_or_else(|| {
                        Error::UnsupportedSabDirectFeature(format!(
                            "assign to stack local `{name}` without a slot type"
                        ))
                    })?;
                    let prim = crate::layout::storage_prim_type(&ty)?;
                    crate::release_prim::emit_store(&mut self.state, slot.reg, 0, src, prim);
                    return Ok(());
                }
                let dst = self
                    .state
                    .frame
                    .local_reg(&name)
                    .ok_or_else(|| Error::MissingType(format!("assign target `{name}` is not a local")))?;
                self.b().emit_assign_reg(dst, src);
                Ok(())
            }
            Node::BlockStmt(b) => self.gen_scoped_block(&b.body),
            Node::ArenaStmt(b) => self.gen_scoped_block(&b.body),
            // R-3 goroutine batch: check-level slice passthrough
            // (see sla-codegen-text stmt.rs note).
            Node::GoStmt(_) | Node::DeferStmt(_) | Node::SelectStmt(_) => Ok(()),
            Node::UsingDecl(_) => Ok(()),
            Node::ExprStmt(e) => {
                // Zig genStmt expr arm: if/switch/match and panic
                // calls discard without release; other calls and
                // expressions evaluate (subset: releases omitted,
                // SAB-D19).
                let _ = self.gen_expr(e)?;
                Ok(())
            }
            Node::ReturnStmt(r) => {
                let value = match &r.value {
                    Some(v) => Some(self.gen_expr(v)?),
                    None => None,
                };
                if !self.b().last_is_terminator() {
                    // CLI-D5R: Zig's return path runs the
                    // checker's per-statement cleanup list
                    // (locals/params still active at the exit,
                    // result root already excluded by the
                    // checker) before the return itself — stock
                    // SAB releases the `limit` param of
                    // `while_sum` exactly here. The text backend
                    // consumes the same list at the same point.
                    self.release_cleanups_for_stmt(stmt)?;
                    self.b().emit_return(value);
                }
                Ok(())
            }
            Node::WhileStmt(w) => self.gen_while(w),
            Node::BreakStmt(_) => self.gen_loop_jump(stmt, true),
            Node::ContinueStmt(_) => self.gen_loop_jump(stmt, false),
            Node::ReleaseStmt(r) => Err(unsupported(
                &format!("explicit release of `{}` (release engine)", r.var_name),
                "E5 release_locals",
                "10210",
            )),
            Node::ForStmt(f) => self.gen_for(f),
            Node::LetElseStmt(_) => Err(unsupported(
                "let-else statement (pattern family)",
                "G patterns",
                "10217-10251",
            )),
            Node::LetDestructureStmt(_) => Err(unsupported(
                "let-destructure statement (pattern family)",
                "G patterns",
                "10253-10320",
            )),
            other => Err(unsupported(
                &format!("statement kind {other:?}"),
                "B4 driver",
                "10213",
            )),
        }
    }

    /// Zig `genLet` + `genLetFromValue` (lines 6916–7020,
    /// 6823–6914), scalar path: evaluate, then bind — a plain
    /// register assign into the binding register, exactly the Zig
    /// fallback arm. String literals route to the D group (slice /
    /// raw-pointer materialization is surface machinery).
    pub(crate) fn gen_let(&mut self, name: &str, _explicit_ty: Option<&Type>, value: &Node) -> Result<()> {
        if sla_lowering::is_discard_name(name) {
            let _ = self.gen_expr(value)?;
            return Ok(());
        }
        // I group: the genLet closure-binding arms (Zig 8156–8180)
        // — a closure literal or a plain alias of a closure
        // binding registers in `closure_bindings` instead of
        // materializing a value.
        if self.try_gen_closure_let(name, value)? {
            return Ok(());
        }
        if matches!(value, Node::Literal(Literal::StringVal(_))) {
            return Err(unsupported(
                "string literal let binding (slice materialization)",
                "D surface / B6 literals",
                "6940-6990",
            ));
        }
        let src = self.gen_expr(value)?;
        if self.b().last_is_terminator() {
            return Ok(());
        }
        let dst = self.binding_reg(name);
        let ty = self.check.expr_type(value).cloned();
        // Zig `genLetFromValue` (6864-6873): a primitive-typed
        // binding that is assigned again later in the function is
        // homed in a stack slot (alloc + store + stack local), so
        // reassignments store through the slot and reads load from
        // it. Without this the binding register is redefined by
        // every assignment, which the verifier rejects.
        if let Some(ty) = &ty {
            if crate::prescan_bindings::binding_needs_scalar_reassign_slot(
                name,
                ty,
                &self.state.program.assigned_bindings,
            ) {
                crate::release_prim::emit_stack_alloc(
                    &mut self.state,
                    dst,
                    crate::layout::type_size(ty),
                );
                let prim = crate::layout::storage_prim_type(ty)?;
                crate::release_prim::emit_store(&mut self.state, dst, 0, src, prim);
                return crate::locals::push_stack_local(&mut self.state, name, dst, ty);
            }
        }
        self.b().emit_assign_reg(dst, src);
        self.state.frame.push_local(match ty {
            Some(ty) => Local::typed(name, dst, false, ty),
            None => Local::plain(name, dst, false),
        });
        Ok(())
    }

    /// Zig `genWhile` (lines 16503–16620), plain-cond path:
    /// head/body/cond_false/exit labels allocated in that order,
    /// branch written with the SAB-D13 duplicated then-label (via
    /// the trunk's `emit_branch`).
    pub(crate) fn gen_while(&mut self, w: &WhileStmt) -> Result<()> {
        if w.let_pattern.is_some() {
            return Err(unsupported(
                "while-let pattern",
                "G patterns",
                "16510-16580",
            ));
        }
        let head = self.b().new_label("L_WHILE_HEAD");
        let body_label = self.b().new_label("L_WHILE_BODY");
        let cond_false = self.b().new_label("L_WHILE_COND_FALSE");
        let exit = self.b().new_label("L_WHILE_EXIT");
        self.b().emit_jmp(&head);
        self.b().emit_label(&head);
        let cond = self.gen_expr(&w.cond)?;
        self.b().emit_branch(cond, &body_label, &cond_false);
        let saved_locals = self.state.frame.locals.len();
        self.b().emit_label(&body_label);
        // CLI-D5R: stock SAB releases the condition temp at the
        // head of *both* successors (body and cond_false) — the
        // branch form (raw release, no bookkeeping), because the
        // same register is retired once per path and the head is
        // re-entered per iteration. A local condition (named
        // or tmp-named, e.g. a loop variable) is not a temp
        // and is left to the cleanup walkers.
        if self.is_releasable_tmp(cond) {
            emit_branch_release(&mut self.state, cond);
        }
        self.state.frame.loop_continue_labels.push(head.clone());
        self.state.frame.loop_break_labels.push(exit.clone());
        let result = self.gen_block(&w.body);
        self.state.frame.loop_continue_labels.pop();
        self.state.frame.loop_break_labels.pop();
        result?;
        if !self.b().last_is_terminator() {
            self.b().emit_jmp(&head);
        }
        self.state.frame.pop_locals_to(saved_locals);
        self.b().emit_label(&cond_false);
        if self.is_releasable_tmp(cond) {
            emit_branch_release(&mut self.state, cond);
        }
        self.b().emit_jmp(&exit);
        self.b().emit_label(&exit);
        Ok(())
    }

    /// Zig `genLoopJump`: statement cleanups, then jmp to the
    /// innermost loop's break / continue label (E5's
    /// `gen_loop_jump`, wired by CLI-D5R).
    pub(crate) fn gen_loop_jump(&mut self, stmt: &Node, is_break: bool) -> Result<()> {
        static NO_DROPS: NoDropFacts = NoDropFacts;
        let mut engine = DropEngine::new(self.check, &NO_DROPS);
        let mut frags = NoMacroFragments;
        let kind = if is_break {
            crate::types::LoopJumpKind::Break
        } else {
            crate::types::LoopJumpKind::Continue
        };
        engine.gen_loop_jump(&mut self.state, &mut frags, stmt, kind)
    }
}
