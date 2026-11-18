//! If-expression emission, plain path, mirrored from codegen.zig
//! (15697-15880). Two counter behaviors matter for byte parity and
//! are preserved exactly: the `L_MERGE` label is allocated even when
//! no merge block is emitted, and a value-less if returns a freshly
//! allocated but discarded temporary register (the invisible tmp_7
//! in fixture 07 — coordinator item D-C5, resolved in
//! docs/divergences/lowering-text.md). The if-let chain path is
//! deferred and fails loudly.

use sla_ast::{Node, Type};
use sla_lowering::{block_terminates, is_discard_name, is_void_type};

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
        // Branch-compensation candidates (codegen.zig
        // `emitBranchConsumptionCompensation`, the β fix): the
        // pre-branch live owned bindings — named locals/params
        // known at branch entry, minus discard names, minus
        // global consts, minus live RefCell borrow handles.
        let live_owned: Vec<String> = self
            .local_binding_types
            .iter()
            .map(|(n, _)| n.clone())
            .filter(|n| {
                !is_discard_name(n)
                    && !self.global_const_bindings.contains(n)
                    && !self.refcell_borrow_handles.contains_key(n)
            })
            .collect();
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
        // The then side's jump (and any compensation releases)
        // are deferred to the compensation step below, which
        // inserts them at this offset.
        let then_text_end = self.out.len();
        let then_end_snapshot = self.snapshot_consumed();
        // The Zig restores the pre-branch state before generating
        // the else side unconditionally (consumed / store-moved /
        // borrow / RefCell books).
        self.restore_consumed(&consumed_snapshot);

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

        if needs_merge {
            self.emit_branch_compensation(
                &live_owned,
                &consumed_snapshot.0,
                &then_end_snapshot.0,
                then_text_end,
                &merge_label,
                then_terminates,
                else_terminates,
            )?;
            // Post-merge settle (codegen.zig, the call-site tail):
            // both sides reach the merge -> consumed is the union
            // of the two end states; the else side terminates ->
            // the then end state survives; the then side
            // terminates -> the else end state (already live).
            if !then_terminates && !else_terminates {
                let union: std::collections::HashSet<String> = then_end_snapshot
                    .0
                    .union(&self.consumed_bindings)
                    .cloned()
                    .collect();
                self.consumed_bindings = union;
            } else if else_terminates {
                self.restore_consumed(&then_end_snapshot);
            }
            self.wl(&format!("{merge_label}:"));
        } else {
            // Both sides terminate: the Zig inserts the deferred
            // blank line at the end of the then segment and prints
            // one after the else segment.
            self.out.insert_str(then_text_end, "\n");
            self.w("\n");
            if else_terminates {
                self.restore_consumed(&then_end_snapshot);
            }
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

    /// codegen.zig:2440 `emitBranchConsumptionCompensation` (the
    /// β fix): a binding consumed on exactly one branch path is
    /// released at the end of the OTHER path, before that path's
    /// jump to the merge — otherwise the merge point sees the
    /// register Active on one predecessor and Consumed on the
    /// other (PhiStateConflict, trap 1015; the reg3 redis_client
    /// / redis_session shape). The Zig store-moved differential
    /// (`emitBranchSettle`) and load-class settle have no state
    /// in this lane (no store-moved set, no load-class tracking);
    /// the family shapes consume through plain calls, which this
    /// core differential path covers.
    fn emit_branch_compensation(
        &mut self,
        live_owned: &[String],
        pre_consumed: &std::collections::HashSet<String>,
        then_consumed: &std::collections::HashSet<String>,
        then_text_end: usize,
        merge_label: &str,
        then_terminates: bool,
        else_terminates: bool,
    ) -> Result<()> {
        let else_consumed = self.consumed_bindings.clone();
        if !then_terminates && !else_terminates {
            // Consumed only on the then path: release at the end
            // of the else path (appended live, before its jump).
            for name in live_owned {
                if then_consumed.contains(name)
                    && !pre_consumed.contains(name)
                    && !else_consumed.contains(name)
                {
                    self.emit_release(name)?;
                }
            }
        }
        if !else_terminates {
            self.w(&format!("    jmp {merge_label}\n"));
        }
        self.w("\n");
        if !then_terminates {
            // Consumed only on the else path: release at the end
            // of the then path — rendered into a scratch buffer
            // and inserted at then_text_end, followed by the
            // then jump and the segment's blank line.
            let live_out = std::mem::take(&mut self.out);
            if !else_terminates {
                for name in live_owned {
                    if else_consumed.contains(name)
                        && !pre_consumed.contains(name)
                        && !then_consumed.contains(name)
                    {
                        self.consumed_bindings.remove(name);
                        self.emit_release(name)?;
                    }
                }
            }
            self.w(&format!("    jmp {merge_label}\n"));
            self.w("\n");
            let scratch = std::mem::replace(&mut self.out, live_out);
            self.out.insert_str(then_text_end, &scratch);
        }
        Ok(())
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
