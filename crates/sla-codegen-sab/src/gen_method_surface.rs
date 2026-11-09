//! D surface (surface_call) — the second half of the S2'
//! method-wall port (see `gen_method_call.rs` for the chain
//! head): `genResultOptionQueryCall` (17564),
//! `genOptionUnwrapCall` (22576), `genUnwrapOrCall` (22721),
//! `genOptionUnwrapOrDefaultCall` (22760),
//! `genAtomicMethodCall` (17353; load/store/fetch_add arms),
//! the bare-call branches of `genStdSurfaceCall` (21345:
//! function / fallible_method / method rules behind the
//! DEF-SAB-SURFACE-GET-SHADOW known-callee guard), and
//! `genVecDequePushBackCall` (22639). Emission helpers are
//! shared from `gen_method_call.rs`. Split solely for the
//! 700-line file discipline; the two files are one
//! decomposition unit.

use sla_ast::{CallExpr, Node, Type};

use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::gen_method_call::{
    atomic_int_macro_prefix, atomic_ordering_arg_token, is_string_type,
};
use crate::ir::PrimType;
use crate::layout::{element_slot_size, prim_type, type_base_name, user_defined_generic_inner};
use crate::surface_query::{
    find_std_surface_rule, std_surface_rule_has_arg, StdSurfaceValues,
};
use crate::types::{StdSurfaceArgKind, StdSurfaceRuleKind};

impl<'c> Driver<'c> {
    // -- Option/Result query + unwrap probes ----------------------

    /// Zig `genResultOptionQueryCall` (17564):
    /// `is_ok`/`is_err` on Result receivers, `is_some`/`is_none`
    /// on Option receivers, as direct macro fragments.
    pub(crate) fn gen_result_option_query_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.associated_target.is_some() || call.args.len() != 1 {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        let (import_path, macro_name) = match call.func_name.as_str() {
            "is_ok" | "is_err" => {
                if user_defined_generic_inner(&receiver_ty, "Result").is_none() {
                    return Ok(None);
                }
                (
                    "sa_std/core/result.sa",
                    if call.func_name == "is_ok" {
                        "RESULT_IS_OK"
                    } else {
                        "RESULT_IS_ERR"
                    },
                )
            }
            "is_some" | "is_none" => {
                if user_defined_generic_inner(&receiver_ty, "Option").is_none() {
                    return Ok(None);
                }
                (
                    "sa_std/core/option.sa",
                    if call.func_name == "is_some" {
                        "OPTION_IS_SOME"
                    } else {
                        "OPTION_IS_NONE"
                    },
                )
            }
            _ => return Ok(None),
        };
        let recv_reg = self.gen_expr(&call.args[0])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let dst_name = self.b().symbol_name(dst).to_string();
        let recv_name = self.b().symbol_name(recv_reg).to_string();
        self.emit_method_macro_fragment(import_path, macro_name, &[dst_name, recv_name])?;
        self.release_non_local(recv_reg)?;
        Ok(Some(dst))
    }

    /// Zig `genOptionUnwrapCall` (22576): `OPTION_UNWRAP`
    /// fragment on Option receivers.
    pub(crate) fn gen_option_unwrap_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.func_name != "unwrap"
            || call.associated_target.is_some()
            || call.args.len() != 1
        {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        if user_defined_generic_inner(&receiver_ty, "Option").is_none() {
            return Ok(None);
        }
        let recv_reg = self.gen_expr(&call.args[0])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let dst_name = self.b().symbol_name(dst).to_string();
        let recv_name = self.b().symbol_name(recv_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/core/option.sa",
            "OPTION_UNWRAP",
            &[dst_name, recv_name],
        )?;
        self.release_non_local(recv_reg)?;
        Ok(Some(dst))
    }

    /// Zig `genUnwrapOrCall` (22721): `unwrap_or(default)` for
    /// Option and Result receivers. The default register is
    /// marked consumed on both branches (DEF-R8-FIX in the Zig
    /// comment): the macro moves it into the result on
    /// None/Err and drops it inside its Some/Ok arm, so no
    /// further instruction may touch it.
    pub(crate) fn gen_unwrap_or_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.func_name != "unwrap_or"
            || call.associated_target.is_some()
            || call.args.len() != 2
        {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        let (import_path, macro_name) =
            if user_defined_generic_inner(&receiver_ty, "Result").is_some() {
                ("sa_std/core/result.sa", "RESULT_UNWRAP_OR")
            } else if user_defined_generic_inner(&receiver_ty, "Option").is_some() {
                ("sa_std/core/option.sa", "OPTION_UNWRAP_OR")
            } else {
                return Ok(None);
            };
        let recv_reg = self.gen_expr(&call.args[0])?;
        let default_reg = self.gen_expr(&call.args[1])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let dst_name = self.b().symbol_name(dst).to_string();
        let recv_name = self.b().symbol_name(recv_reg).to_string();
        let default_name = self.b().symbol_name(default_reg).to_string();
        self.emit_method_macro_fragment(
            import_path,
            macro_name,
            &[dst_name, recv_name, default_name],
        )?;
        self.release_non_local(recv_reg)?;
        self.mark_consumed(default_reg);
        Ok(Some(dst))
    }

    /// Zig `genOptionUnwrapOrDefaultCall` (22760):
    /// `OPTION_UNWRAP_OR_DEFAULT` fragment on Option receivers.
    pub(crate) fn gen_option_unwrap_or_default_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.func_name != "unwrap_or_default"
            || call.associated_target.is_some()
            || call.args.len() != 1
        {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        if user_defined_generic_inner(&receiver_ty, "Option").is_none() {
            return Ok(None);
        }
        let recv_reg = self.gen_expr(&call.args[0])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let dst_name = self.b().symbol_name(dst).to_string();
        let recv_name = self.b().symbol_name(recv_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/core/option.sa",
            "OPTION_UNWRAP_OR_DEFAULT",
            &[dst_name, recv_name],
        )?;
        self.release_non_local(recv_reg)?;
        Ok(Some(dst))
    }

    // -- atomic methods ----------------------------------------------

    /// Zig `genAtomicMethodCall` (17353), load / store /
    /// fetch_add arms (compare_exchange, with its Result fold
    /// and merge bookkeeping, is not ported and stays loud).
    /// Ordering arguments are `Ordering::X` identifiers lowered
    /// through the shared `atomicOrderingToken` table and
    /// passed as macro literal operands.
    pub(crate) fn gen_atomic_method_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.associated_target.is_some() || call.args.is_empty() {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        let prefix = match atomic_int_macro_prefix(&receiver_ty) {
            Some(prefix) => prefix,
            None => {
                if user_defined_generic_inner(&receiver_ty, "AtomicPtr").is_none() {
                    return Ok(None);
                }
                if call.func_name != "load" {
                    return Ok(None);
                }
                "ATOMIC_PTR"
            }
        };
        match call.func_name.as_str() {
            "load" => {
                if call.args.len() != 2 {
                    return Err(unsupported(
                        "atomic load without exactly one ordering argument",
                        "D surface (surface_call)",
                        "17353-17380",
                    ));
                }
                let ordering = atomic_ordering_arg_token(&call.args[1])?;
                let recv_reg = self.gen_expr(&call.args[0])?;
                let dst = self.b().new_tmp();
                self.b().record_reg(dst);
                let dst_name = self.b().symbol_name(dst).to_string();
                let recv_name = self.b().symbol_name(recv_reg).to_string();
                self.emit_method_macro_fragment_literal(
                    "sa_std/sync/atomic.sa",
                    &format!("{prefix}_LOAD"),
                    &[dst_name, recv_name, ordering.to_string()],
                    &[false, false, true],
                )?;
                self.release_non_local(recv_reg)?;
                Ok(Some(dst))
            }
            "store" => {
                if call.args.len() != 3 {
                    return Err(unsupported(
                        "atomic store without value + ordering arguments",
                        "D surface (surface_call)",
                        "17353-17400",
                    ));
                }
                let ordering = atomic_ordering_arg_token(&call.args[2])?;
                let recv_reg = self.gen_expr(&call.args[0])?;
                let value_reg = self.gen_expr(&call.args[1])?;
                let recv_name = self.b().symbol_name(recv_reg).to_string();
                let value_name = self.b().symbol_name(value_reg).to_string();
                self.emit_method_macro_fragment_literal(
                    "sa_std/sync/atomic.sa",
                    &format!("{prefix}_STORE"),
                    &[recv_name, value_name, ordering.to_string()],
                    &[false, false, true],
                )?;
                self.release_non_local(recv_reg)?;
                self.release_non_local(value_reg)?;
                let sentinel = self.b().new_tmp();
                self.b().emit_assign_imm(sentinel, 0);
                Ok(Some(sentinel))
            }
            "fetch_add" => {
                if call.args.len() != 3 {
                    return Err(unsupported(
                        "atomic fetch_add without value + ordering arguments",
                        "D surface (surface_call)",
                        "17353-17420",
                    ));
                }
                let ordering = atomic_ordering_arg_token(&call.args[2])?;
                let recv_reg = self.gen_expr(&call.args[0])?;
                let value_reg = self.gen_expr(&call.args[1])?;
                let dst = self.b().new_tmp();
                self.b().record_reg(dst);
                let dst_name = self.b().symbol_name(dst).to_string();
                let recv_name = self.b().symbol_name(recv_reg).to_string();
                let value_name = self.b().symbol_name(value_reg).to_string();
                self.emit_method_macro_fragment_literal(
                    "sa_std/sync/atomic.sa",
                    &format!("{prefix}_FETCH_ADD"),
                    &[dst_name, recv_name, value_name, ordering.to_string()],
                    &[false, false, false, true],
                )?;
                self.release_non_local(recv_reg)?;
                self.release_non_local(value_reg)?;
                Ok(Some(dst))
            }
            _ => Ok(None),
        }
    }

    // -- std-surface bare-call dispatch ------------------------------

    /// Zig `genStdSurfaceCall` (21345), bare-call branches:
    /// behind the DEF-SAB-SURFACE-GET-SHADOW known-callee
    /// guard, the `function` rules (keyed on the first
    /// argument's type), then `fallible_method`, then `method`.
    /// (The constructor-by-result-type branch that precedes
    /// the guard in Zig serves bare enum constructors, not
    /// receiver methods, and is not part of this slice.)
    pub(crate) fn gen_std_surface_bare_call(
        &mut self,
        _expr: &Node,
        call: &CallExpr,
    ) -> Result<Option<u32>> {
        if call.associated_target.is_some() {
            return Ok(None);
        }
        // DEF-SAB-SURFACE-GET-SHADOW: a call whose name the
        // checker knows as a function resolves to that
        // function; the surface lookups must not shadow it.
        let resolved = self.check.resolve_alias(&call.func_name);
        if self.funcs.contains_key(&resolved)
            || self.check.func(&call.func_name).is_some()
            || self.check.is_extern_func(&call.func_name)
        {
            return Ok(None);
        }
        if call.args.is_empty() {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        let receiver_type_name = match type_base_name(&receiver_ty) {
            Some(name) => name,
            None => return Ok(None),
        };
        // function rules
        let function_rule = find_std_surface_rule(
            &self.state.program.std_surface_rules,
            StdSurfaceRuleKind::Function,
            &receiver_type_name,
            Some(&call.func_name),
        )
        .cloned();
        if let Some(rule) = function_rule {
            let receiver_reg = self.gen_expr(&call.args[0])?;
            let dst = self.b().new_tmp();
            self.b().record_reg(dst);
            let values = StdSurfaceValues {
                out: Some(dst),
                receiver: Some(receiver_reg),
                elem_size: element_slot_size(&receiver_ty),
                ..Default::default()
            };
            self.emit_method_rule(&rule, &values)?;
            self.release_non_local(receiver_reg)?;
            return Ok(Some(dst));
        }
        // fallible_method rules: ok/out pair, panic on !ok.
        let fallible_rule = find_std_surface_rule(
            &self.state.program.std_surface_rules,
            StdSurfaceRuleKind::FallibleMethod,
            &receiver_type_name,
            Some(&call.func_name),
        )
        .cloned();
        if let Some(rule) = fallible_rule {
            let receiver_reg = self.gen_expr(&call.args[0])?;
            let arg_reg = if call.args.len() > 1 {
                Some(self.gen_expr(&call.args[1])?)
            } else {
                None
            };
            let ok_reg = self.b().new_tmp();
            let dst = self.b().new_tmp();
            self.b().record_reg(ok_reg);
            self.b().record_reg(dst);
            let values = StdSurfaceValues {
                out: Some(dst),
                ok: Some(ok_reg),
                receiver: Some(receiver_reg),
                value: arg_reg,
                index: arg_reg,
                elem_size: element_slot_size(&receiver_ty),
                ..Default::default()
            };
            self.emit_method_rule(&rule, &values)?;
            let ok_label = self.b().new_label("L_STD_SURFACE_OK");
            let fail_label = self.b().new_label("L_STD_SURFACE_FAIL");
            // Zig writes the ok label into both branch slots
            // (operands 1 and 2), the fail label into slot 3.
            self.emit_surface_branch(ok_reg, &ok_label, &fail_label);
            self.b().emit_label(&fail_label);
            if !self.is_local_reg(ok_reg) {
                crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
            }
            let panic_code = rule.panic_code.ok_or_else(|| {
                unsupported(
                    "fallible surface rule without a panic code",
                    "D surface (surface_call)",
                    "21490-21510",
                )
            })?;
            self.b().emit_panic_code(panic_code);
            self.b().emit_label(&ok_label);
            if !self.is_local_reg(ok_reg) {
                crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
            }
            self.release_non_local(receiver_reg)?;
            if let Some(reg) = arg_reg {
                self.release_non_local(reg)?;
            }
            return Ok(Some(dst));
        }
        // method rules
        let mut rule = match find_std_surface_rule(
            &self.state.program.std_surface_rules,
            StdSurfaceRuleKind::Method,
            &receiver_type_name,
            Some(&call.func_name),
        )
        .cloned()
        {
            Some(rule) => rule,
            None => return Ok(None),
        };
        // DEF-SAB-AUDIT item 3 (74622): a HashSet<String> keys
        // on string CONTENT, not pointer identity; substitute
        // the *_str rule variants for SET_INSERT / SET_CONTAINS
        // when the value argument is a String.
        if (rule.macro_name == "SET_INSERT" || rule.macro_name == "SET_CONTAINS")
            && call.args.len() > 1
        {
            if let Some(value_ty) = self.check.expr_type(&call.args[1]).cloned() {
                if is_string_type(&value_ty) {
                    if rule.macro_name == "SET_INSERT" {
                        rule.macro_name = "SET_INSERT_STR".to_string();
                        rule.deps = [
                            "sa_set_insert_str",
                            "sa_map_put_str",
                            "map_hash_str_seed",
                            "map_table_probe_str",
                            "map_probe_existing_str",
                            "map_str_key_eq",
                            "map_rehash",
                            "map_table_insert",
                            "sa_mem_set",
                        ]
                        .iter()
                        .map(|dep| dep.to_string())
                        .collect();
                    } else {
                        rule.macro_name = "SET_CONTAINS_STR".to_string();
                        rule.deps = [
                            "sa_set_contains_str",
                            "sa_map_get_str",
                            "map_hash_str_seed",
                            "map_table_probe_str",
                            "map_probe_existing_str",
                            "map_str_key_eq",
                        ]
                        .iter()
                        .map(|dep| dep.to_string())
                        .collect();
                    }
                }
            }
        }
        let receiver_reg = self.gen_expr(&call.args[0])?;
        let value_reg = if call.args.len() > 1 {
            // Zig materializes a string-literal value for
            // BTREE_SET rules through genStringLiteral; the
            // Rust driver's string literal arm is not ported
            // yet, so that shape errors loudly inside gen_expr
            // exactly like every other string value position.
            Some(self.gen_expr(&call.args[1])?)
        } else {
            None
        };
        let has_out = std_surface_rule_has_arg(&rule, StdSurfaceArgKind::Out);
        let dst = if has_out {
            let reg = self.b().new_tmp();
            self.b().record_reg(reg);
            Some(reg)
        } else {
            None
        };
        let values = StdSurfaceValues {
            out: dst,
            receiver: Some(receiver_reg),
            value: value_reg,
            elem_size: element_slot_size(&receiver_ty),
            ..Default::default()
        };
        self.emit_method_rule(&rule, &values)?;
        self.release_non_local(receiver_reg)?;
        if let Some(reg) = value_reg {
            if !self.is_local_reg(reg) {
                let value_ty = self
                    .check
                    .expr_type(&call.args[1])
                    .cloned()
                    .ok_or_else(|| Error::MissingType("surface method value type".to_string()))?;
                if matches!(prim_type(&value_ty), Ok(PrimType::Ptr)) {
                    if self.state.frame.stack_alloc_emitted.contains(&reg) {
                        self.mark_consumed(reg);
                    } else {
                        self.emit_move(reg);
                    }
                } else {
                    self.emit_release(reg)?;
                }
            }
        }
        if let Some(reg) = dst {
            return Ok(Some(reg));
        }
        let sentinel = self.b().new_tmp();
        self.b().record_reg(sentinel);
        self.b().emit_assign_imm(sentinel, 0);
        Ok(Some(sentinel))
    }

    /// Zig `emitBranch` (15334): operands are
    /// `[cond, then_label, then_label, else_label]` (the
    /// duplicated then slot is Zig's shape, mirrored by
    /// `ModuleBuilder::emit_branch`).
    fn emit_surface_branch(&mut self, cond: u32, ok_label: &str, fail_label: &str) {
        self.b().emit_branch(cond, ok_label, fail_label);
    }

    // -- VecDeque push_back ----------------------------------------

    /// Zig `genVecDequePushBackCall` (22639): the element
    /// store register comes from the I-group store-reg helper
    /// ported with `VecDeque::from` in gen_surface_call
    /// (`gen_vec_deque_element_store_reg`). Void: 0 sentinel.
    pub(crate) fn gen_vec_deque_push_back_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.func_name != "push_back" || call.associated_target.is_some() || call.args.len() != 2 {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        let elem_ty = match user_defined_generic_inner(&receiver_ty, "VecDeque") {
            Some(ty) => ty,
            None => return Ok(None),
        };
        self.ensure_method_std_deps("sa_std/vec_deque.sa", &["sa_vec_deque_push_back"])?;
        let deque_reg = self.gen_expr(&call.args[0])?;
        let value_reg = self.gen_expr(&call.args[1])?;
        let stored = self.gen_vec_deque_element_store_reg(&call.args[1], value_reg, &elem_ty)?;
        let deque_name = self.b().symbol_name(deque_reg).to_string();
        let store_name = self.b().symbol_name(stored.0).to_string();
        self.emit_method_macro_fragment(
            "sa_std/vec_deque.sa",
            "VEC_DEQUE_PUSH_BACK",
            &[deque_name, store_name],
        )?;
        self.release_non_local(deque_reg)?;
        if stored.1 {
            self.release_non_local(value_reg)?;
        }
        let sentinel = self.b().new_tmp();
        self.b().record_reg(sentinel);
        self.b().emit_assign_imm(sentinel, 0);
        Ok(Some(sentinel))
    }
}
