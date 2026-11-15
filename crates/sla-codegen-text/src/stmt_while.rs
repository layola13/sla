//! While statement emission, mirrored from codegen.zig's
//! `genWhileStmt` plain path (11431-11545). The while-let / pattern
//! arms route through the pattern-matching family and are deferred:
//! they fail loudly here. Range `for` statements lower onto the
//! same loop skeleton with the hoisted counter slot
//! (`{var}_slot`, emitted at function entry by
//! emit_hoisted_and_counter_slots) as the loop variable's home.

use sla_ast::{Node, Type};
use sla_lowering::block_terminates;

use crate::codegen::{type_string, Codegen};
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// codegen.zig:11431 `genWhileStmt` (plain condition path).
    pub(crate) fn gen_while_stmt(&mut self, stmt: &Node) -> Result<()> {
        let while_stmt = match stmt {
            Node::WhileStmt(w) => w,
            _ => unreachable!(),
        };
        if while_stmt.let_pattern.is_some() {
            return unsupported(
                "while-let statement (pattern family deferred)",
                "codegen.zig:11440 (while let_pattern path)",
            );
        }
        let head_label = self.new_label("L_WHILE_HEAD");
        let body_label = self.new_label("L_WHILE_BODY");
        let cond_false_label = self.new_label("L_WHILE_COND_FALSE");
        let exit_label = self.new_label("L_WHILE_EXIT");

        self.w(&format!("    jmp {head_label}\n\n"));
        self.wl(&format!("{head_label}:"));
        let cond_reg = self.gen_branch_condition_reg(&while_stmt.cond)?;
        self.w(&format!(
            "    br {cond_reg} -> {body_label}, {cond_false_label}\n\n"
        ));
        self.wl(&format!("{body_label}:"));
        // Zig prints the condition release raw (not via emitRelease).
        self.w(&format!("    !{cond_reg}\n"));

        self.loop_continue_labels.push(head_label.clone());
        self.loop_break_labels.push(exit_label.clone());
        // Zig: `defer self.popLoopBodyLocalScope()` — the scope
        // stays live across the back-edge cleanups below and is
        // popped on EVERY exit path, including a gen_block error.
        // The previous shape popped immediately after gen_block
        // (so the cleanups read the OUTER scope or none) and
        // skipped the pop entirely on the error path, leaking a
        // zero depth entry that the enclosing gen_block's
        // leave-tracking then decremented — a subtract-overflow
        // panic masking the real error (14 corpus files).
        self.push_loop_body_local_scope();
        let mut flow = self.gen_block(&while_stmt.body);
        self.loop_continue_labels.pop();
        self.loop_break_labels.pop();

        if flow.is_ok() && !block_terminates(&while_stmt.body) {
            flow = self.emit_active_loop_body_local_cleanups(None, false);
            if flow.is_ok() {
                flow = self.emit_loop_body_top_level_local_cleanups(&while_stmt.body);
            }
            if flow.is_ok() {
                self.w(&format!("    jmp {head_label}\n\n"));
            }
        }
        self.pop_loop_body_local_scope();
        flow?;
        self.wl(&format!("{cond_false_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        self.w(&format!("    jmp {exit_label}\n\n"));
        self.wl(&format!("{exit_label}:"));
        Ok(())
    }

    /// Range `for var in start..end` over the hoisted counter
    /// slot. The collection form (no end) stays loud. The slot
    /// is memory, so the loop has no register-phi surface; the
    /// body runs under the same loop-local scope discipline as
    /// `while`, with `continue` landing on the increment.
    pub(crate) fn gen_for_stmt(&mut self, stmt: &Node) -> Result<()> {
        let for_stmt = match stmt {
            Node::ForStmt(f) => f,
            _ => unreachable!(),
        };
        let Some(end_expr) = &for_stmt.end else {
            return unsupported(
                "for statement over a collection (range form only)",
                "codegen.zig:11300 (for_stmt emission)",
            );
        };
        let slot = format!("{}_slot", for_stmt.var_name);
        let counter_ty = self
            .expr_type(&for_stmt.start)
            .unwrap_or(Type::Primitive(sla_ast::Primitive::I64));
        let ty_str = type_string(&counter_ty);

        let start_reg = self.gen_expr(&for_stmt.start)?;
        self.ins(&format!("store {slot}+0, {start_reg} as {ty_str}"));
        if self.expr_result_reg_needs_release(&for_stmt.start) {
            self.emit_release(&start_reg)?;
        }
        let end_reg = self.gen_expr(end_expr)?;
        let end_is_temp = self.expr_result_reg_needs_release(end_expr);

        let head_label = self.new_label("L_FOR_HEAD");
        let body_label = self.new_label("L_FOR_BODY");
        let incr_label = self.new_label("L_FOR_INCR");
        let cond_false_label = self.new_label("L_FOR_COND_FALSE");
        let exit_label = self.new_label("L_FOR_EXIT");

        self.w(&format!("    jmp {head_label}\n\n"));
        self.wl(&format!("{head_label}:"));
        let cur_reg = self.new_tmp();
        self.ins(&format!("{cur_reg} = load {slot}+0 as {ty_str}"));
        let cond_reg = self.new_tmp();
        let cmp = if matches!(
            counter_ty,
            Type::Primitive(
                sla_ast::Primitive::U8
                    | sla_ast::Primitive::U16
                    | sla_ast::Primitive::U32
                    | sla_ast::Primitive::U64
                    | sla_ast::Primitive::Usize
            )
        ) {
            "ult"
        } else {
            "slt"
        };
        self.ins(&format!("{cond_reg} = {cmp} {cur_reg}, {end_reg}"));
        self.emit_release(&cur_reg)?;
        self.w(&format!(
            "    br {cond_reg} -> {body_label}, {cond_false_label}\n\n"
        ));
        self.wl(&format!("{body_label}:"));
        self.w(&format!("    !{cond_reg}\n"));

        self.loop_continue_labels.push(incr_label.clone());
        self.loop_break_labels.push(exit_label.clone());
        self.push_loop_body_local_scope();
        self.push_binding_alias_to(&for_stmt.var_name, &slot);
        let was_absent = self.addressable_bindings.insert(for_stmt.var_name.clone());
        let mut flow = self.gen_block(&for_stmt.body);
        self.pop_binding_alias(&for_stmt.var_name);
        if was_absent {
            self.addressable_bindings.remove(&for_stmt.var_name);
        }
        self.loop_continue_labels.pop();
        self.loop_break_labels.pop();

        if flow.is_ok() && !block_terminates(&for_stmt.body) {
            flow = self.emit_active_loop_body_local_cleanups(None, false);
            if flow.is_ok() {
                flow = self.emit_loop_body_top_level_local_cleanups(&for_stmt.body);
            }
        }
        if flow.is_ok() {
            // The increment block is the `continue` target: it
            // must exist (and jump back to the head) even when
            // the body always terminates, since a `continue`
            // inside still lands here.
            self.wl(&format!("{incr_label}:"));
            let cur2 = self.new_tmp();
            self.ins(&format!("{cur2} = load {slot}+0 as {ty_str}"));
            let one = self.new_tmp();
            self.ins(&format!("{one} = 1"));
            let next = self.new_tmp();
            self.ins(&format!("{next} = add {cur2}, {one}"));
            self.ins(&format!("store {slot}+0, {next} as {ty_str}"));
            self.emit_release(&cur2)?;
            self.emit_release(&one)?;
            self.emit_release(&next)?;
            self.w(&format!("    jmp {head_label}\n\n"));
        }
        self.pop_loop_body_local_scope();
        flow?;
        self.wl(&format!("{cond_false_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        self.w(&format!("    jmp {exit_label}\n\n"));
        self.wl(&format!("{exit_label}:"));
        if end_is_temp {
            self.emit_release(&end_reg)?;
        }
        Ok(())
    }
}
