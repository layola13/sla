//! Match-expression emission (stage 4). Tag dispatch over the
//! scrutinee with per-arm payload binding loads, mirroring the
//! if-expression's result-slot and phi-cleanup discipline
//! (expr_if.rs). Supported scrutinees: user enums (tag at +0 as
//! i64, payload fields at `layout::enum_field_layout` offsets),
//! Option (tag +0: None=0 / Some=1, payload at +8) and Result
//! (tag +0: Ok=0 / Err=1, ok at +8, err at +16) — the std layouts
//! from sa_std/core/option.sal and result.sal.
//!
//! Payload bindings are emitted through the binding-alias stack:
//! each binding name gets a fresh register per arm, so two arms
//! binding the same source name never collide, and arm-body
//! identifiers resolve through `resolve_binding_name`. Phi
//! cleanup names attached to an arm body's last statement are
//! printed through the same resolution (the raw source names are
//! not registers on this path).
//!
//! Ownership scope of this first form: payload loads are plain
//! loads. An identifier scrutinee's own cleanup stays with the
//! checker-recorded statement cleanups. A temporary scrutinee is
//! released at the merge only when no arm binds a non-Copy
//! payload (a moved-out heap payload would otherwise be released
//! twice); with a non-Copy payload the temp scrutinee is left
//! unreleased rather than double-released.

use sla_ast::{MatchCase, Node, Type};
use sla_lowering::{block_terminates, is_void_type};

use crate::layout;

use crate::codegen::{type_size, type_string, Codegen};
use crate::{Result, TextError};

/// One arm's dispatch plan: the tag value to compare against
/// (None = unconditional / wildcard), and the payload loads as
/// (binding name, offset, payload type).
struct ArmPlan {
    tag: Option<i64>,
    loads: Vec<(String, usize, Type)>,
}

enum ScrutClass {
    Enum(sla_ast::EnumDecl),
    Option,
    Result,
}

fn unsupported<T>(feature: String) -> Result<T> {
    Err(TextError::Unsupported {
        feature,
        zig_ref: "codegen.zig genMatchExpr (stage 4 port)",
    })
}

impl<'c> Codegen<'c> {
    fn match_class(&self, scrut_ty: &Type) -> Result<ScrutClass> {
        if let Type::UserDefined(u) = scrut_ty {
            match u.name.as_str() {
                "Option" => return Ok(ScrutClass::Option),
                "Result" => return Ok(ScrutClass::Result),
                _ => {}
            }
            if let Some(decl) = self.enum_decl(&u.name) {
                return Ok(ScrutClass::Enum(decl.clone()));
            }
        }
        unsupported(format!(
            "match scrutinee type {}",
            type_string(scrut_ty)
        ))
    }

    fn match_arm_plan(&self, class: &ScrutClass, scrut_ty: &Type, case: &MatchCase) -> Result<ArmPlan> {
        let pat = &case.pattern;
        let wildcard = pat.variant_name.is_empty() || pat.variant_name == "_";
        if wildcard {
            // A catch-all binds the scrutinee itself (handled by
            // the caller through the alias path; loads stay empty).
            return Ok(ArmPlan {
                tag: None,
                loads: Vec::new(),
            });
        }
        match class {
            ScrutClass::Option => {
                let inner = match scrut_ty {
                    Type::UserDefined(u) => u.generics.first().cloned().unwrap_or(Type::Infer),
                    _ => Type::Infer,
                };
                match pat.variant_name.as_str() {
                    "None" => Ok(ArmPlan {
                        tag: Some(0),
                        loads: Vec::new(),
                    }),
                    "Some" => Ok(ArmPlan {
                        tag: Some(1),
                        loads: pat
                            .bindings
                            .iter()
                            .filter(|b| b.as_str() != "_")
                            .map(|b| (b.clone(), 8, inner.clone()))
                            .collect(),
                    }),
                    other => unsupported(format!("match Option variant `{other}`")),
                }
            }
            ScrutClass::Result => {
                let (ok_ty, err_ty) = match scrut_ty {
                    Type::UserDefined(u) => (
                        u.generics.first().cloned().unwrap_or(Type::Infer),
                        u.generics.get(1).cloned().unwrap_or(Type::Infer),
                    ),
                    _ => (Type::Infer, Type::Infer),
                };
                match pat.variant_name.as_str() {
                    "Ok" => Ok(ArmPlan {
                        tag: Some(0),
                        loads: pat
                            .bindings
                            .iter()
                            .filter(|b| b.as_str() != "_")
                            .map(|b| (b.clone(), 8, ok_ty.clone()))
                            .collect(),
                    }),
                    "Err" => Ok(ArmPlan {
                        tag: Some(1),
                        loads: pat
                            .bindings
                            .iter()
                            .filter(|b| b.as_str() != "_")
                            .map(|b| (b.clone(), 16, err_ty.clone()))
                            .collect(),
                    }),
                    other => unsupported(format!("match Result variant `{other}`")),
                }
            }
            ScrutClass::Enum(decl) => {
                let tag = match layout::enum_variant_value(decl, &pat.variant_name) {
                    Some(t) => t,
                    None => {
                        return unsupported(format!(
                            "match variant {}::{}",
                            decl.name, pat.variant_name
                        ))
                    }
                };
                let variant = match decl.variants.iter().find(|v| v.name == pat.variant_name) {
                    Some(v) => v,
                    None => {
                        return unsupported(format!(
                            "match variant {}::{}",
                            decl.name, pat.variant_name
                        ))
                    }
                };
                let mut loads = Vec::new();
                for (idx, binding) in pat.bindings.iter().enumerate() {
                    if binding == "_" {
                        continue;
                    }
                    // Struct-style variants bind by field name;
                    // tuple-style variants bind positionally.
                    let field = match variant
                        .fields
                        .iter()
                        .find(|f| &f.name == binding)
                        .or_else(|| variant.fields.get(idx))
                    {
                        Some(f) => f,
                        None => {
                            return unsupported(format!(
                                "match binding `{binding}` has no field in {}::{}",
                                decl.name, pat.variant_name
                            ))
                        }
                    };
                    let (offset, field_ty) = match layout::enum_field_layout(variant, &field.name) {
                        Some(v) => v,
                        None => {
                            return unsupported(format!(
                                "match field layout `{}`",
                                field.name
                            ))
                        }
                    };
                    loads.push((binding.clone(), offset, field_ty));
                }
                Ok(ArmPlan {
                    tag: Some(tag),
                    loads,
                })
            }
        }
    }

    pub(crate) fn gen_match_expr(&mut self, expr: &Node) -> Result<String> {
        let m = match expr {
            Node::MatchExpr(m) => m,
            _ => unreachable!(),
        };
        let scrut_ty: Type = self.require_expr_type(&m.val, "match scrutinee")?;
        let class = self.match_class(&scrut_ty)?;
        let mut plans = Vec::new();
        for case in &m.cases {
            plans.push(self.match_arm_plan(&class, &scrut_ty, case)?);
        }
        let any_noncopy_payload = plans
            .iter()
            .flat_map(|p| p.loads.iter())
            .any(|(_, _, ty)| !self.type_is_copy_value(ty));

        let scrut_reg = self.gen_expr(&m.val)?;
        let scrut_is_temp = !matches!(&*m.val, Node::Identifier(_));
        let scrut_needs_release = scrut_is_temp
            && !any_noncopy_payload
            && self.expr_result_reg_needs_release(&m.val);

        let expr_ty: Type = self.require_expr_type(expr, "match result")?;
        let has_value = !is_void_type(&expr_ty);
        let result_slot: Option<String> = if has_value {
            let slot = self.new_tmp();
            self.ins(&format!("{slot} = alloc {}", type_size(&expr_ty)));
            Some(slot)
        } else {
            None
        };
        let n = m.cases.len();
        let merge_label = self.new_label("L_MATCH_MERGE");
        let test_labels: Vec<String> = (0..n).map(|_| self.new_label("L_MATCH_TEST")).collect();
        let arm_labels: Vec<String> = (0..n).map(|_| self.new_label("L_MATCH_ARM")).collect();
        let needs_tag = plans.iter().any(|p| p.tag.is_some());
        let tag_reg: Option<String> = if needs_tag {
            let tag = self.new_tmp();
            self.ins(&format!("{tag} = load {scrut_reg}+0 as i64"));
            Some(tag)
        } else {
            None
        };

        let consumed_snapshot = self.snapshot_consumed();
        let mut any_nonterminating = false;
        for (i, case) in m.cases.iter().enumerate() {
            let plan = &plans[i];
            let next_label = if i + 1 < n {
                test_labels[i + 1].clone()
            } else {
                merge_label.clone()
            };
            self.wl(&format!("{}:", test_labels[i]));
            let cmp_reg: Option<String> = match (plan.tag, &tag_reg) {
                (Some(tag_value), Some(tag)) => {
                    let cmp = self.new_tmp();
                    self.ins(&format!("{cmp} = eq {tag}, {tag_value}"));
                    // The mismatch edge gets its own block so the
                    // compare register is Consumed on every path
                    // into the next test (the arm edge releases it
                    // at the arm head; a release at the next test's
                    // head would see Untracked vs Consumed from its
                    // two predecessors and trap PhiStateConflict).
                    let miss_label = self.new_label("L_MATCH_MISS");
                    self.w(&format!("    br {cmp} -> {}, {miss_label}\n\n", arm_labels[i]));
                    self.wl(&format!("{miss_label}:"));
                    self.w(&format!("    !{cmp}\n"));
                    self.w(&format!("    jmp {next_label}\n"));
                    Some(cmp)
                }
                _ => {
                    self.w(&format!("    jmp {}\n", arm_labels[i]));
                    None
                }
            };
            self.w("\n");
            self.wl(&format!("{}:", arm_labels[i]));
            if let Some(cmp) = &cmp_reg {
                self.w(&format!("    !{cmp}\n"));
            }
            self.restore_consumed(&consumed_snapshot);
            // Payload bindings.
            let mut pushed: Vec<String> = Vec::new();
            let pat = &case.pattern;
            let wildcard = pat.variant_name.is_empty() || pat.variant_name == "_";
            if wildcard {
                for binding in &pat.bindings {
                    if binding == "_" {
                        continue;
                    }
                    self.push_binding_alias_to(binding, &scrut_reg);
                    self.remember_local_binding_type(binding, &scrut_ty);
                    pushed.push(binding.clone());
                }
            } else {
                for (binding, offset, payload_ty) in &plan.loads {
                    let alias = self.push_binding_alias(binding);
                    let load_ty = match payload_ty {
                        Type::Infer => "u64".to_string(),
                        t => type_string(t).to_string(),
                    };
                    self.ins(&format!("{alias} = load {scrut_reg}+{offset} as {load_ty}"));
                    self.remember_local_binding_type(&alias, payload_ty);
                    pushed.push(binding.clone());
                }
            }
            // Guard: a false guard falls through to the next test,
            // retiring the payload bindings on the fail path first
            // (they were loaded in this arm's test block and would
            // otherwise reach the merge Active from this edge only).
            if let Some(guard) = &case.guard {
                let greg = self.gen_expr(guard)?;
                let body_label = self.new_label("L_MATCH_BODY");
                let fail_label = self.new_label("L_MATCH_GUARD_FAIL");
                self.w(&format!("    br {greg} -> {body_label}, {fail_label}\n\n"));
                self.wl(&format!("{fail_label}:"));
                self.w(&format!("    !{greg}\n"));
                // The fail block is a separate runtime path: emit
                // its releases under a snapshot so the body path's
                // own arm-exit releases are not suppressed.
                let fail_snapshot = self.snapshot_consumed();
                for binding in &pushed {
                    self.emit_release(binding)?;
                }
                self.restore_consumed(&fail_snapshot);
                self.w(&format!("    jmp {next_label}\n\n"));
                self.wl(&format!("{body_label}:"));
                self.w(&format!("    !{greg}\n"));
            }
            let terminates = block_terminates(&case.body);
            match (&result_slot, terminates) {
                (Some(slot), false) => {
                    self.gen_block_tail_value_store(&case.body, slot, &expr_ty)?;
                }
                _ => {
                    self.gen_block(&case.body)?;
                }
            }
            self.emit_match_phi_cleanups(case.body.last())?;
            // Arm-scope exit (stage 5b checker-plans): pattern
            // bindings are not statements, so no checker cleanup
            // retires them; the checker recorded each arm's
            // still-active pattern bindings during check_match and
            // the emitter prints that list through emit_release
            // (which no-ops for consumed / stack / borrow registers
            // and dedups against the phi cleanups just printed).
            // A binding the body consumed — directly, or through a
            // re-alias let move the emitted text cannot see — is
            // absent from the list. Wildcard bindings alias the
            // scrutinee itself and stay with the scrutinee's own
            // cleanup plan. When the projection carries nothing
            // for this match (a harness CheckInfo without arm
            // data), nothing is printed: the checker is
            // authoritative, and the old text-scan approximation
            // (`^alias` / `return alias` substring search over the
            // emitted body) is gone.
            if !wildcard {
                if let Some(list) = self.check.match_arm_exit_cleanups(expr, i) {
                    for name in list.to_vec() {
                        self.emit_release(&name)?;
                    }
                }
            }
            for binding in pushed.iter().rev() {
                self.pop_binding_alias(binding);
            }
            if !terminates {
                any_nonterminating = true;
                self.w(&format!("    jmp {merge_label}\n"));
            } else {
                self.restore_consumed(&consumed_snapshot);
            }
            self.w("\n");
        }
        self.wl(&format!("{merge_label}:"));
        if let Some(tag) = &tag_reg {
            self.w(&format!("    !{tag}\n"));
        }
        if scrut_needs_release {
            self.emit_release(&scrut_reg)?;
        }
        if !any_nonterminating {
            self.restore_consumed(&consumed_snapshot);
            // Every arm terminated: the merge is reachable only
            // from the final test's mismatch edge, which the
            // checker's exhaustiveness verdict makes dead. Close
            // the block with a trap terminator so the function
            // never falls through, and hand back a discarded tmp
            // (the value is unreachable, mirroring the void-if
            // counter behavior).
            self.ins("panic(2)");
            return Ok(self.new_tmp());
        }
        match result_slot {
            Some(slot) => {
                let result = self.new_tmp();
                self.ins(&format!("{result} = load {slot}+0 as {}", type_string(&expr_ty)));
                self.emit_release(&slot)?;
                Ok(result)
            }
            None => Ok(self.new_tmp()),
        }
    }

    /// Phi cleanups for a match arm body: the names are
    /// source-level binding names, resolved through the alias
    /// stack before printing (unlike the if path, where the names
    /// are already registers).
    fn emit_match_phi_cleanups(&mut self, last_stmt: Option<&Node>) -> Result<()> {
        if let Some(stmt) = last_stmt {
            if let Some(list) = self.check.phi_cleanups(stmt) {
                let names: Vec<String> = list.to_vec();
                for name in names {
                    let reg = self.resolve_binding_name(&name);
                    self.w(&format!("    !{reg}\n"));
                    // Mark it settled so the arm-exit payload
                    // releases do not print a second release.
                    self.consumed_bindings.insert(reg);
                }
            }
        }
        Ok(())
    }
}
