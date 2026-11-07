//! While statement emission, mirrored from codegen.zig's
//! `genWhileStmt` plain path (11431-11545). The while-let / pattern
//! arms route through the pattern-matching family and are deferred:
//! they fail loudly here.

use sla_ast::Node;
use sla_lowering::block_terminates;

use crate::codegen::Codegen;
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
        self.push_loop_body_local_scope();
        self.gen_block(&while_stmt.body)?;
        self.pop_loop_body_local_scope();
        self.loop_continue_labels.pop();
        self.loop_break_labels.pop();

        if !block_terminates(&while_stmt.body) {
            self.emit_active_loop_body_local_cleanups(None, false)?;
            self.emit_loop_body_top_level_local_cleanups(&while_stmt.body)?;
            self.w(&format!("    jmp {head_label}\n\n"));
        }
        self.wl(&format!("{cond_false_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        self.w(&format!("    jmp {exit_label}\n\n"));
        self.wl(&format!("{exit_label}:"));
        Ok(())
    }
}
