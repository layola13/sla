//! B4 (continued) — statement/expression generation for the
//! driver subset: the `gen*` arms `driver.rs`'s orchestration
//! dispatches into. Zig ranges are cited per method, as in
//! `driver.rs`. Split from `driver.rs` solely for the 700-line
//! file discipline; the two files are one decomposition unit (B4).

use sla_ast::{BinaryOp, CallExpr, IfExpr, Literal, Node, Primitive, Type, WhileStmt};
use sla_lowering::{is_panic_builtin_name, is_println_call, plan_scalar_binary_op};

use crate::error::{Error, Result};
use crate::ir::Operand;
use crate::release_drop::NoDropFacts;
use crate::release_emit::{emit_branch_release, emit_release, NoMacroFragments};
use crate::release_locals::DropEngine;
use crate::types::Local;
use crate::driver::{scalar_binary_op_kind, unsupported, Driver};

impl<'c> Driver<'c> {
    // -- release wiring (CLI-D5R) ------------------------------------
    // The E-group release engine existed but the driver never
    // consumed it (SAB-D19 item 1): stock SAB releases every
    // non-local temporary at its consumption point and runs the
    // checker's cleanup lists at returns / loop jumps, while the
    // driver emitted none of those, so `sa test`'s strict verifier
    // reported MemoryLeak on the first unconsumed temp. The
    // helpers below wire exactly the consumption points the
    // scalar subset reaches, mirroring the stock disassembly of
    // the CLI-D5 repro shape (`while_sum` + panic-guard tests).

    /// A register is a releasable temporary when it is not a
    /// named local/param: the builder names every such register
    /// `tmp_<n>` (literals, op results, call results, loads).
    pub(crate) fn is_tmp_reg(&self, reg: u32) -> bool {
        // Registers with no interned name in this builder
        // (foreign substitution regs) are never classified as
        // temps — they are some other frame's named values.
        self.state
            .builder
            .symbol_name_opt(reg)
            .map(|name| name.starts_with("tmp_"))
            .unwrap_or(false)
    }

    /// A register is a *releasable* temp when it is tmp-named
    /// and not currently bound as a local: the for-loop
    /// variable is a tmp-named local, and `is_local_reg` wins
    /// over the name.
    pub(crate) fn is_releasable_tmp(&self, reg: u32) -> bool {
        self.is_tmp_reg(reg) && !crate::captures::is_local_reg(&self.state, reg)
    }

    /// Zig `emitRelease` at a consumption point: release a
    /// releasable temporary register (locals/params are
    /// retired by the checker-cleanup walkers, not here).
    pub(crate) fn release_tmp(&mut self, reg: u32) -> Result<()> {
        if self.is_releasable_tmp(reg) {
            let mut frags = NoMacroFragments;
            emit_release(&mut self.state, &mut frags, reg)?;
        }
        Ok(())
    }

    /// Zig `releaseCleanupForStmt` via the E5 engine over this
    /// driver's checker facts: run the checker's cleanup list
    /// recorded for `stmt` (return / break / continue exits).
    /// `NoDropFacts` is the registered loud default: a planned
    /// deep drop is unavailable, so cleanup locals take the
    /// shallow release path (Zig's `catch return false` shape in
    /// `emit_planned_local_drop`).
    pub(crate) fn release_cleanups_for_stmt(&mut self, stmt: &Node) -> Result<()> {
        static NO_DROPS: NoDropFacts = NoDropFacts;
        let mut engine = DropEngine::new(self.check, &NO_DROPS);
        let mut frags = NoMacroFragments;
        engine.release_cleanup_for_stmt(&mut self.state, &mut frags, stmt)
    }
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
            Node::VarStmt(v) => Err(unsupported(
                &format!("var statement `{}` (stack slot materialization)", v.name),
                "E2 locals",
                "10145-10149",
            )),
            Node::AssignStmt(a) => {
                let name = match &*a.target {
                    Node::Identifier(name) => name.clone(),
                    _ => {
                        return Err(unsupported(
                            "assign to non-identifier target",
                            "G gen_assign",
                            "10150",
                        ))
                    }
                };
                let dst = self
                    .state
                    .frame
                    .local_reg(&name)
                    .ok_or_else(|| Error::MissingType(format!("assign target `{name}` is not a local")))?;
                let src = self.gen_expr(&a.value)?;
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
        self.b().emit_assign_reg(dst, src);
        let ty = self.check.expr_type(value).cloned();
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

    // -- expressions ------------------------------------------------

    /// Zig `genExpr` (lines 10565–10740), subset arms.
    pub(crate) fn gen_expr(&mut self, expr: &Node) -> Result<u32> {
        match expr {
            Node::Literal(lit) => self.gen_literal(lit, expr),
            Node::Identifier(name) => {
                // Zig genExpr identifier arm (12264): a closure
                // parameter currently being inlined resolves to
                // its substituted register before any local.
                if let Some(&mapped) = self.state.program.closure_param_regs.get(name) {
                    return Ok(mapped);
                }
                if let Some(reg) = self.state.frame.local_reg(name) {
                    return Ok(reg);
                }
                if let Some(node) = self.state.program.global_scalar_consts.get(name).cloned() {
                    if let Node::Literal(lit) = node {
                        return self.gen_literal(&lit, expr);
                    }
                    return Err(unsupported(
                        "global const chain",
                        "C1 prescan_consts",
                        "10590-10595",
                    ));
                }
                Ok(self.b().intern(name))
            }
            Node::BinaryExpr(bin) => {
                if bin.op == BinaryOp::Spaceship {
                    return Err(unsupported(
                        "spaceship comparison (Ordering struct lowering)",
                        "G gen_spaceship",
                        "10827",
                    ));
                }
                let lhs_ty = self.check.expr_type(&bin.left).cloned();
                let rhs_ty = self.check.expr_type(&bin.right).cloned();
                if matches!(lhs_ty, Some(Type::UserDefined(_)))
                    || matches!(rhs_ty, Some(Type::UserDefined(_)))
                {
                    return Err(unsupported(
                        "binary op over struct operands (field-wise families)",
                        "G struct ops",
                        "10830-10860",
                    ));
                }
                // Types: checker facts first; literal operands fall
                // back to the other side / i64 (the SAB-D12 fallback
                // shape, kept only where a literal pins it).
                let lt = lhs_ty.or_else(|| literal_fallback_type(&bin.left));
                let rt = rhs_ty.or_else(|| literal_fallback_type(&bin.right));
                let lt = lt.or_else(|| rt.clone()).ok_or_else(|| {
                    Error::MissingType("binary op left operand type".to_string())
                })?;
                let rt = rt.or_else(|| Some(lt.clone())).unwrap();
                let plan = plan_scalar_binary_op(bin.op, &lt, &rt).ok_or_else(|| {
                    unsupported("scalar binary op has no plan", "E1 opkinds", "2370-2380")
                })?;
                let lhs = self.gen_expr(&bin.left)?;
                let rhs = self.gen_expr(&bin.right)?;
                let dst = self.b().new_tmp();
                self.b()
                    .emit_op(dst, scalar_binary_op_kind(plan), Operand::Reg(lhs), Operand::Reg(rhs));
                // CLI-D5R: an SAB `op` reads but does not consume
                // its operands — stock SAB releases each operand
                // temp right after the op (loads, literals and
                // call results alike; named locals/params are
                // skipped by `release_tmp`). This is the point
                // that retired the CLI-D5 repro's leaked call
                // result in the panic-guard tests.
                self.release_tmp(lhs)?;
                if rhs != lhs {
                    self.release_tmp(rhs)?;
                }
                Ok(dst)
            }
            Node::CallExpr(call) => self.gen_call(expr, call),
            Node::IfExpr(ife) => self.gen_if(expr, ife),
            Node::CastExpr(c) => self.gen_cast(c),
            Node::UnsafeExpr(u) => self.gen_unsafe_expr(expr, u),
            Node::TryExpr(t) => self.gen_try(expr, t),
            Node::InlineAsmExpr(_) => self.gen_inline_asm(),
            // I group: aggregate literal materialization (Zig
            // genExpr dispatch, current-head sab_codegen.zig
            // 12345–12349 + enum literal arm).
            Node::StructLiteral(lit) => self.gen_struct_literal(lit),
            Node::TupleLiteral(lit) => self.gen_tuple_literal(expr, lit),
            Node::ArrayLiteral(lit) => self.gen_array_literal(expr, lit),
            Node::RepeatArrayLiteral(lit) => self.gen_repeat_array_literal(expr, lit),
            Node::EnumLiteral(lit) => self.gen_enum_literal(lit),
            Node::MoveExpr(mv) => self.gen_move_expr(mv),
            Node::ClosureLiteral(_) => Err(Error::UnsupportedSabDirectFeature(
                // Zig's genExpr has no closure-literal arm: a bare
                // closure literal in expression position falls to
                // the switch's else and is refused plainly (it is
                // only meaningful at a let binding or a call site,
                // both handled by the I-group closure family).
                "closure literal in expression position".to_string(),
            )),
            other => Err(unsupported(
                &format!("expression kind {other:?}"),
                "G gen_expr families",
                "10565-10740",
            )),
        }
    }

    /// Zig `genLiteralTyped` (lines 10741–10775), non-string arms.
    /// String literals need the D-group surface materialization
    /// (slice vs raw pointer), so they stop loudly — except as a
    /// direct `panic`/surface argument, which never reaches here.
    pub(crate) fn gen_literal(&mut self, lit: &Literal, expr: &Node) -> Result<u32> {
        let ty = self.check.expr_type(expr);
        match lit {
            Literal::IntVal(v) => {
                let reg = self.b().new_tmp();
                let unsigned = ty.map(sla_lowering::is_unsigned_integer_type).unwrap_or(false);
                self.b().emit_assign_int(reg, *v, unsigned);
                Ok(reg)
            }
            Literal::FloatVal(v) => {
                let reg = self.b().new_tmp();
                self.b().emit_assign_float(reg, *v);
                Ok(reg)
            }
            Literal::BoolVal(v) => {
                let reg = self.b().new_tmp();
                self.b().emit_assign_imm(reg, if *v { 1 } else { 0 });
                Ok(reg)
            }
            Literal::StringVal(_) => Err(unsupported(
                "string literal value (slice/raw-pointer materialization)",
                "D surface / B6 literals",
                "10750-10760",
            )),
        }
    }

    /// Zig `genCall` (lines 15159–15580) dispatch head + the plain
    /// static-call tail (`emitPlannedStaticCall`), subset form.
    /// Every named surface in the Zig dispatch head that belongs to
    /// another group stops loudly with that attribution.
    pub(crate) fn gen_call(&mut self, _expr: &Node, call: &CallExpr) -> Result<u32> {
        // I group: an immediately-invoked closure callee inlines
        // through the closure family (Zig 17770–17774).
        if let Some(callee) = &call.callee {
            return self.gen_invoked_closure_call(callee, call);
        }
        // panic builtin (Zig 15167-15180): a panic instruction, then
        // a fresh bookkeeping tmp as the expression's register.
        if is_panic_builtin_name(&call.func_name) && call.associated_target.is_none() {
            match call.args.as_slice() {
                [Node::Literal(Literal::IntVal(code))] => {
                    self.b().emit_panic_code(*code);
                }
                [arg] => {
                    let code = self.gen_expr(arg)?;
                    // Non-literal panic code: Zig writes operands[0]
                    // as the code register.
                    self.b().emit_panic_reg(code);
                }
                _ => {
                    // Zig's else arm silently writes code "1" for
                    // any non-single-arg panic (args are dropped
                    // unevaluated) — mirrored byte-for-byte and
                    // registered as SAB-D22 / FB-017.
                    self.b().emit_panic_code(1);
                }
            }
            let sentinel = self.b().new_tmp();
            self.b().record_reg(sentinel);
            return Ok(sentinel);
        }
        if is_println_call(call) {
            // D group format_print, ported for CLI-D4 in
            // gen_print.rs (genPrintlnCall family); the two
            // string-value plans stop loudly there.
            return self.gen_println_call(call);
        }
        if call.func_name == "str_eq" || call.func_name == "STR_EQ" {
            return Err(unsupported(
                "str_eq call (string surface expansion)",
                "D surface (surface_call)",
                "15198-15200",
            ));
        }
        if call.associated_target.is_some() {
            return Err(unsupported(
                &format!(
                    "associated/method call `{}::{}` (surface or method lowering)",
                    call.associated_target.as_deref().unwrap_or("?"),
                    call.func_name
                ),
                "D surface (surface_call)",
                "15260-15440",
            ));
        }
        if self.check.is_macro(&call.func_name) {
            return Err(unsupported(
                &format!("macro call `{}`", call.func_name),
                "G macros",
                "15220-15230",
            ));
        }
        if self.check.is_extern_func(&call.func_name) {
            return Err(unsupported(
                &format!("extern call `{}` (contract extern emission)", call.func_name),
                "B4 driver step 7 / F fragments",
                "15460-15480",
            ));
        }
        // I group: a call through a let-bound closure name
        // inlines the closure body (Zig 17812).
        if call.associated_target.is_none() {
            if let Some(reg) = self.try_gen_bound_closure_call(call)? {
                return Ok(reg);
            }
        }
        // Plain user-function static call.
        let resolved = self.check.resolve_alias(&call.func_name);
        let (arity, is_void) = match self.funcs.get(&resolved) {
            Some(shape) => (shape.arity, shape.is_void),
            None => {
                return Err(unsupported(
                    &format!("call to unknown or non-user function `{}`", call.func_name),
                    "D surface (surface_call) if std, else G gen_call",
                    "15442-15450",
                ))
            }
        };
        if arity != call.args.len() {
            return Err(Error::UnsupportedSabDirectFeature(format!(
                "call to `{}` with {} args, declared arity {} [group B4 driver; Zig sab_codegen.zig:15451]",
                call.func_name,
                call.args.len(),
                arity
            )));
        }
        // C3 `loweredFuncSymbol` (Zig 953–963) over the checker
        // facts: main / extern / no-mangle / `sa_`-prefixed names
        // stay as-is, other user functions become `sla__<name>`.
        let lowered = crate::symbols::lowered_func_symbol(&resolved, self.check);
        let mut arg_regs = Vec::with_capacity(call.args.len());
        for arg in &call.args {
            arg_regs.push(self.gen_expr(arg)?);
        }
        let rendered: Vec<String> = arg_regs
            .iter()
            .map(|&reg| self.b().symbol_name(reg).to_string())
            .collect();
        let body = format!("@{}({})", lowered, rendered.join(", "));
        if is_void {
            self.b().emit_call(None, &body);
            // Zig emitPlannedCallBody void arm: sentinel tmp = 0.
            let sentinel = self.b().new_tmp();
            self.b().emit_assign_imm(sentinel, 0);
            Ok(sentinel)
        } else {
            let dst = self.b().new_tmp();
            self.b().emit_call(Some(dst), &body);
            Ok(dst)
        }
    }

    /// Zig `genIf` / `genIfStatement` / `genIfValue` (lines
    /// 18934–19186), subset form. Statement form: then/else/merge
    /// plus the two exit labels, allocated in Zig's order
    /// (L_THEN, L_ELSE, L_MERGE, L_THEN_EXIT, L_ELSE_EXIT); branches
    /// that fall through jump to their exit label, which jumps to
    /// merge. Value form (both blocks end in an expression
    /// statement and the checker typed the if): each branch assigns
    /// its tail value into a shared result register.
    pub(crate) fn gen_if(&mut self, expr: &Node, ife: &IfExpr) -> Result<u32> {
        if ife.let_chain.is_some() {
            return Err(unsupported(
                "if-let chain",
                "G patterns",
                "18737-18933",
            ));
        }
        // Zig genIf dispatch (lines 19161-19171): a checker-typed
        // non-void if whose both blocks end in an expression
        // statement takes the genIfValue path (alloc/store/load
        // result slot) — not ported in this subset, so it stops
        // loudly instead of being silently mis-lowered as a
        // statement if.
        let tail_is_expr = |block: &[Node]| matches!(block.last(), Some(Node::ExprStmt(_)));
        if let Some(else_block) = &ife.else_block {
            if tail_is_expr(&ife.then_block) && tail_is_expr(else_block) {
                if let Some(ty) = self.check.expr_type(expr).cloned() {
                    if !sla_lowering::is_void_type(&ty) {
                        return self.gen_if_value(ife, else_block, &ty);
                    }
                }
            }
        }
        let cond = self.gen_expr(&ife.cond)?;
        let then_label = self.b().new_label("L_THEN");
        let else_label = self.b().new_label("L_ELSE");
        let merge_label = self.b().new_label("L_MERGE");
        let then_exit = self.b().new_label("L_THEN_EXIT");
        let else_exit = self.b().new_label("L_ELSE_EXIT");
        self.b().emit_branch(cond, &then_label, &else_label);

        let saved_locals = self.state.frame.locals.len();
        self.b().emit_label(&then_label);
        self.gen_block(&ife.then_block)?;
        let then_terminated = self.b().last_is_terminator();
        if !then_terminated {
            self.b().emit_jmp(&then_exit);
        }
        self.state.frame.pop_locals_to(saved_locals);

        self.b().emit_label(&else_label);
        if let Some(else_block) = &ife.else_block {
            self.gen_block(else_block)?;
        }
        let else_terminated = self.b().last_is_terminator();
        if !else_terminated {
            self.b().emit_jmp(&else_exit);
        }
        self.state.frame.pop_locals_to(saved_locals);

        if !then_terminated {
            self.b().emit_label(&then_exit);
            self.b().emit_jmp(&merge_label);
        }
        if !else_terminated {
            self.b().emit_label(&else_exit);
            self.b().emit_jmp(&merge_label);
        }
        if !then_terminated || !else_terminated {
            self.b().emit_label(&merge_label);
        }
        // Zig genIfStatement returns a fresh bookkeeping tmp as the
        // expression's register (no defining assignment).
        let result = self.b().new_tmp();
        self.b().record_reg(result);
        Ok(result)
    }
}


/// The SAB-D12 literal fallback, narrowed: a literal operand's own
/// shape supplies its type when the checker recorded none (int ->
/// i64 per Zig's `.integer`, float -> f64, bool -> boolean). Any
/// other shape supplies nothing and the caller fails MissingType.
fn literal_fallback_type(node: &Node) -> Option<Type> {
    match node {
        Node::Literal(Literal::IntVal(_)) => Some(Type::Primitive(Primitive::I64)),
        Node::Literal(Literal::FloatVal(_)) => Some(Type::Primitive(Primitive::F64)),
        Node::Literal(Literal::BoolVal(_)) => Some(Type::Primitive(Primitive::Boolean)),
        _ => None,
    }
}

