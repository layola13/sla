//! `select` statement emission for the text lane (goroutine
//! syntax batch), mirroring syntax_lowering.zig `lowerSelect`.
//!
//! In the Zig pipeline a select never reaches codegen as a
//! select: the lowering rewrites it into ordinary statements —
//! entry evaluation of every arm's channel handle (and send
//! payload) in arm order, flat handles / ops arrays, one flat
//! 16-bytes-per-case slot array with the send payloads staged
//! into their slots, a single `sa_chan_select` call gated by
//! the rc check (panic 60916), and an exhaustive dispatch
//! chain on the out-index (timeout -2, default -1, case i)
//! terminated by a trap (panic 60919) instead of an empty
//! fallthrough. The text lane has no syntax-lowering stage,
//! so the same instruction sequence is emitted here directly,
//! in the lowering's order and with its panic codes.
//!
//! Register discipline (proven by the CHANNEW / FG3 lines and
//! by the lowering's own DEF-TEXT31-SELECT retire notes):
//! the arrays and out-cells are `stack_alloc` cells (never
//! explicitly released); every materialised temp is settled
//! on every path that can reach a return or the statement
//! end — the dispatch tests reload the out-index from its
//! cell per branch instead of holding it in a register, and
//! each test's condition register is retired at the head of
//! both successor blocks, so a `return` inside an arm body
//! never strands a synthetic register.

use sla_ast::stmts::{SelectArm, SelectStmt};
use sla_ast::{Node, Primitive, Type};
use sla_lowering::{call_arg_needs_release, stmt_terminates};

use crate::chan::ChanMethod;
use crate::codegen::{type_string, Codegen};
use crate::{Result, TextError};

struct CaseInfo {
    kind: ChanMethod,
    payload_ty: Type,
}

impl<'c> Codegen<'c> {
    fn select_case_call<'a>(&self, arm: &'a SelectArm) -> Result<(&'a sla_ast::CallExpr, CaseInfo)> {
        let recv = arm.recv.as_deref().ok_or_else(|| TextError::Unsupported {
            feature: "select case without a call".to_string(),
            zig_ref: "syntax_lowering.zig lowerSelect (checker-recorded chan call)",
        })?;
        let call = match recv {
            Node::CallExpr(c) => c,
            _ => {
                return Err(TextError::Unsupported {
                    feature: "select case call is not a call expression".to_string(),
                    zig_ref: "parser.zig parseSelectStmt (case call must be a call)",
                })
            }
        };
        let (kind, payload_ty) = self
            .chan_method(call)
            .filter(|(k, _)| matches!(k, ChanMethod::Send | ChanMethod::Recv))
            .ok_or_else(|| TextError::Unsupported {
                feature: format!("select case is not a chan send/recv: {}", call.func_name),
                zig_ref: "parser.zig parseSelectStmt (case call must be recv/send)",
            })?;
        Ok((call, CaseInfo { kind, payload_ty }))
    }

    /// syntax_lowering.zig `lowerSelect`, emitted directly.
    pub(crate) fn gen_select_stmt(&mut self, stmt: &Node, sel: &SelectStmt) -> Result<()> {
        // Partition arms: `default` is an arm in this AST (Zig
        // keeps it beside the cases); the timeout arm is the
        // statement's own field. Arm positions in `sel.arms` are
        // tracked alongside: the checker's per-arm exit-cleanup
        // lists are indexed by that position (timeout arm at
        // `sel.arms.len()`).
        let mut cases: Vec<&SelectArm> = Vec::new();
        let mut case_arm_index: Vec<usize> = Vec::new();
        let mut default_body: Option<&Vec<Node>> = None;
        let mut default_arm_index: Option<usize> = None;
        for (arm_idx, arm) in sel.arms.iter().enumerate() {
            if arm.is_default {
                if default_body.is_none() {
                    default_body = Some(&arm.body);
                    default_arm_index = Some(arm_idx);
                }
            } else {
                cases.push(arm);
                case_arm_index.push(arm_idx);
            }
        }
        let n = cases.len();

        // Decompose every case call before emitting anything.
        let mut infos: Vec<CaseInfo> = Vec::new();
        for arm in &cases {
            let (_, info) = self.select_case_call(arm)?;
            infos.push(info);
        }

        // Entry evaluation in arm order (lowerSelect): bind
        // each arm's handle, and stage each send arm's payload
        // value into a register; the slot stores happen once
        // the slot array exists, exactly as the lowering first
        // binds `__sel_sv_` lets and stores them at slot
        // staging time.
        let mut handle_regs: Vec<String> = Vec::new();
        let mut handle_release: Vec<bool> = Vec::new();
        let mut send_vals: Vec<Option<String>> = Vec::new();
        for (i, arm) in cases.iter().enumerate() {
            let (call, _) = self.select_case_call(arm)?;
            let handle = self.gen_expr(&call.args[0])?;
            handle_release.push(call_arg_needs_release(&call.args[0]));
            handle_regs.push(handle);
            if infos[i].kind == ChanMethod::Send {
                let val = self.gen_expr(&call.args[1])?;
                send_vals.push(Some(val));
            } else {
                send_vals.push(None);
            }
        }

        // Handles / ops arrays (stack cells; contents are word
        // copies, the handle registers stay the bindings' own).
        let handles = self.new_tmp();
        self.ins(&format!("{handles} = stack_alloc {}", 8 * n.max(1)));
        for (i, h) in handle_regs.iter().enumerate() {
            self.ins(&format!("store {handles}+{}, {h} as u64", 8 * i));
        }
        for (i, h) in handle_regs.iter().enumerate() {
            if handle_release[i] {
                self.emit_release(h)?;
            }
        }
        let ops = self.new_tmp();
        self.ins(&format!("{ops} = stack_alloc {}", 4 * n.max(1)));
        for (i, info) in infos.iter().enumerate() {
            let op = if info.kind == ChanMethod::Send { 1 } else { 0 };
            self.ins(&format!("store {ops}+{}, {op} as u32", 4 * i));
        }

        // Flat slot array, zero-filled (the lowering's slot
        // array node is zero-filled, so a closed-channel
        // receive reads a zero payload).
        let slots = self.new_tmp();
        self.ins(&format!("{slots} = stack_alloc {}", 16 * n.max(1)));
        for k in 0..(2 * n) {
            self.ins(&format!("store {slots}+{}, 0 as u64", 8 * k));
        }

        // Stage send payloads into their slots (the send arm
        // of emitChanCall, at the slot offset).
        for (i, info) in infos.iter().enumerate() {
            if info.kind != ChanMethod::Send {
                continue;
            }
            let (call, _) = self.select_case_call(cases[i])?;
            let val = send_vals[i].clone().expect("send value evaluated");
            let payload_ty = &info.payload_ty;
            let off = 16 * i;
            if matches!(payload_ty, Type::Pointer(_) | Type::FnPtr(_))
                || self.chan_payload_is_word(payload_ty)
            {
                self.ins(&format!(
                    "store {slots}+{off}, {val} as {}",
                    type_string(payload_ty)
                ));
                if matches!(payload_ty, Type::Pointer(_) | Type::FnPtr(_))
                    || self.type_is_copy_value(payload_ty)
                {
                    if call_arg_needs_release(&call.args[1]) {
                        self.emit_release(&val)?;
                    }
                } else {
                    self.emit_forget_moved_value(&val)?;
                }
            } else {
                let size = self
                    .chan_payload_storage_size(payload_ty)
                    .ok_or_else(|| TextError::Unsupported {
                        feature: format!("chan select send payload size for {payload_ty:?}"),
                        zig_ref: "type_checker.zig payloadStorageSize",
                    })?;
                let addr = self.new_tmp();
                self.ins(&format!("{addr} = ptr_add {slots}, {off}"));
                self.ins(&format!("call @sa_mem_copy(&{addr}, &{val}, {size})"));
                // Task 89: the staging address is a fresh temp —
                // retire it (the word branch retires its value
                // the same way); without this the select-send
                // arm leaks the temp (host MemoryLeak 1012).
                self.emit_release(&addr)?;
                self.emit_forget_moved_value(&val)?;
            }
        }

        // Out cells and the select call.
        let idx_cell = self.new_tmp();
        self.ins(&format!("{idx_cell} = stack_alloc 8"));
        self.ins(&format!("store {idx_cell}+0, 0 as i64"));
        let closed_cell = self.new_tmp();
        self.ins(&format!("{closed_cell} = stack_alloc 8"));
        self.ins(&format!("store {closed_cell}+0, 0 as u32"));
        let (timeout_reg, timeout_release) = match &sel.timeout {
            Some(t) => {
                let reg = self.gen_expr_in_context(
                    &t.expr,
                    Some(&Type::Primitive(Primitive::I64)),
                )?;
                (reg, call_arg_needs_release(&t.expr))
            }
            None => ("-1".to_string(), false),
        };
        let has_default = if default_body.is_some() { 1 } else { 0 };
        // The runtime contract spells the three array
        // parameters `*cases` / `*ops` / `*slots`: a
        // `*`-spelled parameter is passed in deref form at
        // the call site (the sa_chan_send `*value` precedent
        // in chan.rs), unlike the `&out` borrows.
        let rc = self.new_tmp();
        self.ins(&format!(
            "{rc} = call @sa_chan_select(*{handles}, *{ops}, *{slots}, {n}, {has_default}, {timeout_reg}, &{idx_cell}, &{closed_cell})"
        ));
        self.emit_chan_rc_check(&rc, 60916)?;
        self.emit_release(&rc)?;
        if timeout_release {
            self.emit_release(&timeout_reg)?;
        }

        // Dispatch chain in the lowering's branch order
        // (timeout, default, cases in arm order), terminated
        // by the trap branch.
        struct Branch<'x> {
            value: i64,
            case: Option<usize>,
            body: &'x Vec<Node>,
            arm_index: usize,
        }
        let mut branches: Vec<Branch> = Vec::new();
        if let Some(t) = &sel.timeout {
            branches.push(Branch {
                value: -2,
                case: None,
                body: &t.body,
                arm_index: sel.arms.len(),
            });
        }
        if let Some(body) = default_body {
            branches.push(Branch {
                value: -1,
                case: None,
                body,
                arm_index: default_arm_index.unwrap_or(sel.arms.len()),
            });
        }
        for (i, arm) in cases.iter().enumerate() {
            branches.push(Branch {
                value: i as i64,
                case: Some(i),
                body: &arm.body,
                arm_index: case_arm_index[i],
            });
        }

        let end_label = self.new_label("L_SEL_END");
        let trap_label = self.new_label("L_SEL_TRAP");
        let mut any_fallthrough = false;
        for (k, branch) in branches.iter().enumerate() {
            let body_label = self.new_label("L_SEL_BODY");
            let next_label = if k + 1 < branches.len() {
                self.new_label("L_SEL_NEXT")
            } else {
                trap_label.clone()
            };
            let t = self.new_tmp();
            self.ins(&format!("{t} = load {idx_cell}+0 as i64"));
            let cond = self.new_tmp();
            self.ins(&format!("{cond} = eq {t}, {}", branch.value));
            self.emit_release(&t)?;
            self.ins(&format!("br {cond} -> {body_label}, {next_label}"));
            // Body block: retire the condition first (the br
            // read it; both successors own one retire each).
            // The retirement is a RAW write, not emit_release:
            // the register is live on both outgoing edges and
            // the linear release bookkeeping would suppress
            // the second block's copy (the if-expression phi
            // discipline in expr_if.rs does the same).
            self.wl(&format!("{body_label}:"));
            self.w(&format!("    !{cond}\n"));
            if let Some(i) = branch.case {
                if infos[i].kind == ChanMethod::Recv {
                    self.gen_select_recv_binding(cases[i], &infos[i], i, &slots, &closed_cell)?;
                }
            }
            self.gen_scoped_block(branch.body)?;
            let body_terminates = branch
                .body
                .last()
                .map(|s| stmt_terminates(s))
                .unwrap_or(false);
            if !body_terminates {
                // Arm-scope exit (checker-plans, mirroring the
                // match-arm plan in expr_match.rs): the arm's
                // pattern bindings are not statements, so no
                // statement cleanup retires them; the checker
                // recorded each arm's still-active bindings
                // during check_select_stmt and the emitter prints
                // that list here, before the jump to the select
                // end (emit_release no-ops for consumed / stack /
                // borrow registers).
                if let Some(list) = self
                    .check
                    .select_arm_exit_cleanups(stmt, branch.arm_index)
                {
                    for name in list.to_vec() {
                        self.emit_release(&name)?;
                    }
                }
                any_fallthrough = true;
                self.ins(&format!("jmp {end_label}"));
            }
            if k + 1 < branches.len() {
                self.wl(&format!("{next_label}:"));
                self.w(&format!("    !{cond}\n"));
            }
        }
        self.wl(&format!("{trap_label}:"));
        self.ins("panic(60919)");
        // The end label exists only when some branch actually
        // falls through to it; when every branch terminates,
        // the chain ends at the trap, exactly like the lowered
        // Zig chain (an empty trailing block would be a
        // FallthroughForbidden at a function tail).
        if any_fallthrough {
            self.wl(&format!("{end_label}:"));
        }
        Ok(())
    }

    /// Bind a recv arm's pattern names from the arm's slot
    /// region (lowerSelect's recv branch prologue): the
    /// payload name takes the slot's contents — a word load
    /// for word payloads, the slot address itself for
    /// address-resident payloads (the lowering's in-place
    /// reinterpretation) — and the closed name takes
    /// `closed != 0` from the select's out-closed word.
    fn gen_select_recv_binding(
        &mut self,
        arm: &SelectArm,
        info: &CaseInfo,
        case_index: usize,
        slots: &str,
        closed_cell: &str,
    ) -> Result<()> {
        let (v_name, closed_name) = match &arm.pattern {
            Some((v, c)) => (v, c),
            None => return Ok(()),
        };
        let off = 16 * case_index;
        let payload_ty = &info.payload_ty;
        let v_reg = self.resolve_binding_name(v_name);
        self.remember_local_binding_type(&v_reg, payload_ty);
        if self.chan_payload_is_word(payload_ty) {
            self.ins(&format!(
                "{v_reg} = load {slots}+{off} as {}",
                type_string(payload_ty)
            ));
        } else {
            // Address-resident payload: the binding register
            // holds the slot address itself (field projections
            // compute over it, as they do over any aggregate
            // value in this lane).
            self.ins(&format!("{v_reg} = ptr_add {slots}, {off}"));
        }
        if let Some(closed_name) = closed_name {
            let c_reg = self.resolve_binding_name(closed_name);
            self.remember_local_binding_type(
                &c_reg,
                &Type::Primitive(Primitive::Boolean),
            );
            let cw = self.new_tmp();
            self.ins(&format!("{cw} = load {closed_cell}+0 as u32"));
            self.ins(&format!("{c_reg} = ne {cw}, 0"));
            self.emit_release(&cw)?;
        }
        Ok(())
    }
}
