//! If-expression emission, plain path, mirrored from codegen.zig
//! (15697-15880). Two counter behaviors matter for byte parity and
//! are preserved exactly: the `L_MERGE` label is allocated even when
//! no merge block is emitted, and a value-less if returns a freshly
//! allocated but discarded temporary register (the invisible tmp_7
//! in fixture 07 — coordinator item D-C5, resolved in
//! docs/divergences/lowering-text.md). The if-let chain path is
//! deferred and fails loudly.

use sla_ast::{Node, Type};
use sla_lowering::{block_terminates, is_void_type};

use crate::codegen::{type_size, type_string, Codegen};
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// codegen.zig:15697 plain `genIfExpr` path.
    pub(crate) fn gen_if_expr(&mut self, expr: &Node) -> Result<String> {
        let ife = match expr {
            Node::IfExpr(i) => i,
            _ => unreachable!(),
        };
        if ife.let_chain.is_some() {
            return unsupported(
                "if-let chain expression (pattern family deferred)",
                "codegen.zig:15560 (genIfLetChain path)",
            );
        }
        let cond_reg = self.gen_branch_condition_reg(&ife.cond)?;
        let expr_ty: Type = self.require_expr_type(expr, "if expression result")?;
        let has_value = !is_void_type(&expr_ty);

        let result_slot: Option<String> = if has_value {
            let slot = self.new_tmp();
            self.ins(&format!("{slot} = alloc {}", type_size(&expr_ty)));
            Some(slot)
        } else {
            None
        };
        let then_label = self.new_label("L_THEN");
        let else_label = self.new_label("L_ELSE");
        // Allocated unconditionally in the Zig; emitted only when a
        // merge is needed.
        let merge_label = self.new_label("L_MERGE");

        let then_terminates = block_terminates(&ife.then_block);
        let else_terminates = match &ife.else_block {
            Some(block) => block_terminates(block),
            None => false,
        };
        let needs_merge = !then_terminates || !else_terminates;

        let consumed_snapshot = self.snapshot_consumed();
        self.w(&format!(
            "    br {cond_reg} -> {then_label}, {else_label}\n\n"
        ));
        self.wl(&format!("{then_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        match (&result_slot, then_terminates) {
            (Some(slot), false) => {
                self.gen_block_tail_value_store(&ife.then_block, slot, &expr_ty)?;
            }
            _ => {
                self.gen_block(&ife.then_block)?;
            }
        }
        self.emit_phi_cleanups(ife.then_block.last())?;
        if needs_merge && !then_terminates {
            self.w(&format!("    jmp {merge_label}\n"));
        }
        self.w("\n");
        if then_terminates {
            self.restore_consumed(&consumed_snapshot);
        }

        self.wl(&format!("{else_label}:"));
        self.w(&format!("    !{cond_reg}\n"));
        let else_body: &[Node] = ife.else_block.as_deref().unwrap_or(&[]);
        match (&result_slot, else_terminates) {
            (Some(slot), false) => {
                self.gen_block_tail_value_store(else_body, slot, &expr_ty)?;
            }
            _ => {
                self.gen_block(else_body)?;
            }
        }
        self.emit_phi_cleanups(else_body.last())?;
        if needs_merge && !else_terminates {
            self.w(&format!("    jmp {merge_label}\n"));
        }
        self.w("\n");
        if else_terminates {
            self.restore_consumed(&consumed_snapshot);
        }

        if needs_merge {
            self.wl(&format!("{merge_label}:"));
        }
        match result_slot {
            Some(slot) => {
                let result = self.new_tmp();
                self.ins(&format!(
                    "{result} = load {slot}+0 as {}",
                    type_string(&expr_ty)
                ));
                self.emit_release(&slot)?;
                Ok(result)
            }
            // Void if: the Zig returns `try self.newTmp()` — a counter
            // allocation with no emission (D-C5).
            None => Ok(self.new_tmp()),
        }
    }

    /// Phi cleanup registers attached to a block's last statement are
    /// printed raw (not via emitRelease), mirroring the Zig.
    fn emit_phi_cleanups(&mut self, last_stmt: Option<&Node>) -> Result<()> {
        if let Some(stmt) = last_stmt {
            if let Some(list) = self.check.phi_cleanups(stmt) {
                let names: Vec<String> = list.to_vec();
                for name in names {
                    self.w(&format!("    !{name}\n"));
                }
            }
        }
        Ok(())
    }
}
