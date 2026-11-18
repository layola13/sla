//! Switch-expression emission, mirrored from codegen.zig's
//! `genExpr` `.switch_expr` arm (local-sa tree 3de91932; the arm is
//! the equality-ladder form the SAB `emitSwitchCaseCondition`
//! mirrors as well). The scrutinee is evaluated once; a ladder of
//! `L_CHECK_i` blocks compares it against each case pattern in
//! order (fieldless enum-literal patterns against an enum scrutinee
//! compare the discriminant tag loaded at +0, never a freshly
//! materialized literal); a `default` case jumps straight to its
//! body. Case bodies are plain blocks with no pattern bindings.
//! The merge block is emitted only when some path can reach it
//! (Zig: `any_fallthrough or !has_default or !last_is_default`).
//! Like the Zig arm, the switch yields a freshly allocated but
//! discarded temporary — it produces no value slot.

use sla_ast::{Node, Type};
use sla_lowering::block_terminates;
use sla_lowering::names::is_switch_default_pattern;

use crate::codegen::Codegen;
use crate::layout;
use crate::Result;

impl<'c> Codegen<'c> {
    /// codegen.zig `genExpr` `.switch_expr` arm.
    pub(crate) fn gen_switch_expr(&mut self, expr: &Node) -> Result<String> {
        let swe = match expr {
            Node::SwitchExpr(s) => s,
            _ => unreachable!(),
        };
        if swe.cases.is_empty() {
            return Ok(self.new_tmp());
        }
        let val_reg = self.gen_expr(&swe.val)?;
        let val_needs_release = self.expr_result_reg_needs_release(&swe.val);
        let merge_label = self.new_label("L_SWITCH_MERGE");

        // A fieldless enum literal pattern must compare the
        // scrutinee discriminant tag, not the scrutinee pointer
        // against a freshly allocated literal (SAB parity note in
        // the Zig arm).
        let enum_decl = match self.expr_type(&swe.val) {
            Some(Type::UserDefined(u)) => self.enum_decl(&u.name).cloned(),
            _ => None,
        };

        let n = swe.cases.len();
        let case_labels: Vec<String> = (0..n).map(|_| self.new_label("L_CASE")).collect();
        let check_labels: Vec<String> = (0..n).map(|_| self.new_label("L_CHECK")).collect();

        // Jump to the first check.
        self.w(&format!("    jmp {}\n\n", check_labels[0]));

        let consumed_snapshot = self.snapshot_consumed();
        let mut previous_cond: Option<String> = None;
        let mut any_fallthrough = false;
        let mut has_default = false;
        for (idx, case) in swe.cases.iter().enumerate() {
            // -- equality check block --
            self.wl(&format!("{}:", check_labels[idx]));
            if let Some(cond) = previous_cond.take() {
                self.w(&format!("    !{cond}\n"));
            }
            let is_default = is_switch_default_pattern(&case.pattern);
            if is_default {
                has_default = true;
            }
            let mut is_eq = String::new();
            if !is_default {
                is_eq = self.new_tmp();
                let tag_compared = match (&enum_decl, &*case.pattern) {
                    (Some(decl), Node::EnumLiteral(lit))
                        if lit.fields.is_empty()
                            && (lit.enum_name == decl.name
                                || lit.enum_name.rsplit("::").next() == Some(decl.name.as_str())) =>
                    {
                        match layout::enum_variant_value(decl, &lit.variant_name) {
                            Some(tag) => {
                                let tag_reg = self.new_tmp();
                                self.ins(&format!("{tag_reg} = load {val_reg}+0 as i64"));
                                let tag_const = self.new_tmp();
                                self.emit_int_const(&tag_const, tag);
                                self.ins(&format!("{is_eq} = eq {tag_reg}, {tag_const}"));
                                self.w(&format!("    !{tag_reg}\n"));
                                self.w(&format!("    !{tag_const}\n"));
                                true
                            }
                            None => false,
                        }
                    }
                    _ => false,
                };
                if !tag_compared {
                    let pat_reg = self.gen_expr(&case.pattern)?;
                    self.ins(&format!("{is_eq} = eq {val_reg}, {pat_reg}"));
                    if self.expr_result_reg_needs_release(&case.pattern) {
                        self.emit_release(&pat_reg)?;
                    }
                }
            }
            let next_chk = if idx + 1 < n {
                check_labels[idx + 1].clone()
            } else {
                merge_label.clone()
            };
            if is_default {
                self.w(&format!("    jmp {}\n\n", case_labels[idx]));
            } else {
                self.w(&format!(
                    "    br {is_eq} -> {}, {next_chk}\n\n",
                    case_labels[idx]
                ));
                previous_cond = Some(is_eq.clone());
            }

            // -- case body block --
            self.wl(&format!("{}:", case_labels[idx]));
            self.restore_consumed(&consumed_snapshot);
            if !is_default {
                self.w(&format!("    !{is_eq}\n"));
            }
            let terminates = block_terminates(&case.body);
            if val_needs_release && terminates {
                self.emit_release(&val_reg)?;
            }
            self.gen_block(&case.body)?;
            if !terminates {
                self.w(&format!("    jmp {merge_label}\n"));
                any_fallthrough = true;
            } else {
                self.restore_consumed(&consumed_snapshot);
            }
            self.w("\n");
        }

        // The last non-default check branches to the merge label, so
        // the merge block stays whenever it can be referenced.
        let last_is_default = swe
            .cases
            .last()
            .map(|c| is_switch_default_pattern(&c.pattern))
            .unwrap_or(false);
        let need_merge = any_fallthrough || !has_default || !last_is_default;
        if need_merge {
            self.wl(&format!("{merge_label}:"));
            if val_needs_release {
                self.emit_release(&val_reg)?;
            }
        }
        Ok(self.new_tmp())
    }
}
