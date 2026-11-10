//! D surface (surface_call) — the map method probes of Zig
//! `genCallInner` (sab_codegen.zig, current head), ported for
//! the S2' method-wall slice: `genMapInsertCall` (21833),
//! `genMapGetCall` (21974), `genBTreeMapGetCall` (22080),
//! `genMapContainsKeyCall` (22103). The Option folds use the
//! vec-pop branch-merge idiom (clone / restore / merge over
//! the frame's released set), exactly as in Zig.
//!
//! Not ported (stay loud): `genMapClearCall` (its HashMap arm
//! needs the `emitHashMapValueCellsFree` slot walk),
//! `genMapRemoveCall` / `genBTreeMapRemoveCall` (tombstone
//! slot walk), `genMapKeysCall` / `genMapValuesCall`, and
//! string-literal keys (`genMapKeyReg`'s cached
//! `genStringLiteral` arm — the Rust driver's string
//! materialization is not ported yet, so that shape stops
//! loudly here instead of being approximated).

use sla_ast::{CallExpr, Literal, Node, Type};

use crate::branch_state::{clone_released, restore_released, set_merge_released};
use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::gen_method_call::{is_string_type, map_types};
use crate::ir::{Operand, PrimType};
use crate::layout::{prim_type, user_defined_generic_inner};

impl<'c> Driver<'c> {
    /// The ported map probe chain, in Zig `genCallInner`
    /// order. `Ok(None)` = no ported probe claimed the call.
    pub(crate) fn gen_map_method_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if let Some(reg) = self.gen_map_insert_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_map_get_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_btree_map_get_call(call)? {
            return Ok(Some(reg));
        }
        if let Some(reg) = self.gen_map_contains_key_call(call)? {
            return Ok(Some(reg));
        }
        Ok(None)
    }

    /// Zig `genMapKeyReg` (21785): non-literal keys generate
    /// plainly; string-literal keys take Zig's cached
    /// `genStringLiteral` arm, which is not ported — loud stop.
    /// Shared with the IndexExpr read slice (`gen_index`'s
    /// `gen_map_index`).
    pub(crate) fn gen_map_key_reg(&mut self, arg: &Node) -> Result<u32> {
        if let Node::Literal(Literal::StringVal(_)) = arg {
            return Err(unsupported(
                "string-literal map key (string materialization)",
                "D surface (surface_call)",
                "21785-21795",
            ));
        }
        self.gen_expr(arg)
    }

    /// Zig `insertEnumPayloadIsCopy` (21795).
    fn insert_enum_payload_is_copy(&self, ty: &Type) -> bool {
        let Type::UserDefined(ud) = ty else {
            return false;
        };
        let Some(decl) = self.enums.get(&ud.name) else {
            return false;
        };
        decl.variants.iter().all(|variant| {
            variant
                .fields
                .iter()
                .all(|field| self.check.type_is_copy_value(&field.ty))
        })
    }

    /// Zig `insertArgCheckerConsumed` (21814): true when the
    /// checker consumed this insert argument (the map adopted
    /// the argument's handle as-is), so the source register
    /// must be retired with a consumed marker at the insert
    /// site (DEF-INSERT-GETMISS).
    fn insert_arg_checker_consumed(&mut self, recv_arg: &Node, arg: &Node) -> bool {
        let Node::Identifier(arg_name) = arg else {
            return false;
        };
        let arg_ty = match self.check.expr_type(arg).cloned() {
            Some(ty) => ty,
            None => return false,
        };
        if matches!(arg_ty, Type::Infer) {
            return false;
        }
        let copy_leaf = sla_lowering::type_is_copy_value_leaf(&arg_ty).unwrap_or(false)
            || matches!(arg_ty, Type::Pointer(_));
        if !insert_arg_transfers_ownership(&arg_ty, copy_leaf) {
            return false;
        }
        if user_defined_generic_inner(&arg_ty, "Slice").is_some() {
            return false;
        }
        if self.insert_enum_payload_is_copy(&arg_ty) {
            return false;
        }
        if let Type::UserDefined(ud) = &arg_ty {
            if ud.name == "Chan" {
                return false;
            }
        }
        if let Some(recv_id) = sla_lowering::root_identifier(recv_arg) {
            if recv_id == arg_name.as_str() {
                return false;
            }
        }
        true
    }

    /// Zig `genMapInsertCall` (21833): run the insert, then
    /// fold replaced/old into an Option. HashMap values stage
    /// through a per-entry HEAP cell (DEF-HASHMAP-XTALK
    /// A-form); BTreeMap values pass as the u64 payload with a
    /// stack slot staging.
    fn gen_map_insert_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.args.len() != 3 || call.func_name != "insert" || call.associated_target.is_some() {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        let plan = match map_insert_plan(&receiver_ty) {
            Some(plan) => plan,
            None => return Ok(None),
        };
        let recv_reg = self.gen_expr(&call.args[0])?;
        let key_arg = call.args[1].clone();
        let key_reg = self.gen_map_key_reg(&key_arg)?;
        let value_reg = self.gen_expr(&call.args[2])?;
        let value_arg_ty = self.check.expr_type(&call.args[2]).cloned();
        let value_u64 = self.b().new_tmp();
        match &value_arg_ty {
            Some(vty) => {
                let vprim = prim_type(vty)?;
                if vprim != PrimType::U64 {
                    let op = crate::gen_struct::op_kind_for_cast(vprim, PrimType::U64)?;
                    self.b().emit_op(
                        value_u64,
                        op,
                        Operand::Reg(value_reg),
                        Operand::Ty(PrimType::U64 as u32),
                    );
                } else {
                    self.b().emit_op(
                        value_u64,
                        crate::ir::OpKind::Bitcast,
                        Operand::Reg(value_reg),
                        Operand::Ty(PrimType::U64 as u32),
                    );
                }
            }
            None => {
                self.b().emit_op(
                    value_u64,
                    crate::ir::OpKind::Bitcast,
                    Operand::Reg(value_reg),
                    Operand::Ty(PrimType::U64 as u32),
                );
            }
        }
        self.release_non_local(value_reg)?;
        let value_slot = self.b().new_tmp();
        if plan.value_by_slot {
            // DEF-HASHMAP-XTALK (A-form): per-entry heap cell,
            // owned by the map after the insert fragment.
            self.emit_alloc(value_slot, 8);
        } else {
            crate::release_prim::emit_stack_alloc(&mut self.state, value_slot, 8);
            self.mark_non_owning(value_slot);
        }
        self.emit_store(value_slot, 0, value_u64, PrimType::U64);
        self.ensure_method_std_deps(plan.import_path, &[plan.dep_symbol])?;
        let replaced = self.b().new_tmp();
        let old_val = self.b().new_tmp();
        let option_reg = self.b().new_tmp();
        self.b().record_reg(option_reg);
        let replaced_name = self.b().symbol_name(replaced).to_string();
        let old_name = self.b().symbol_name(old_val).to_string();
        let recv_name = self.b().symbol_name(recv_reg).to_string();
        let key_name = self.b().symbol_name(key_reg).to_string();
        let (replaced2, old2, recv2, key2) = (
            replaced_name.clone(),
            old_name.clone(),
            recv_name.clone(),
            key_name.clone(),
        );
        if plan.value_by_slot {
            let slot_name = self.b().symbol_name(value_slot).to_string();
            self.emit_method_macro_fragment(
                plan.import_path,
                plan.macro_name,
                &[replaced_name, old_name, recv_name, key_name, slot_name],
            )?;
        } else {
            let u64_name = self.b().symbol_name(value_u64).to_string();
            self.emit_method_macro_fragment(
                plan.import_path,
                plan.macro_name,
                &[replaced2, old2, recv2, key2, u64_name],
            )?;
        }
        self.emit_release(value_u64)?;
        if plan.value_by_slot {
            self.emit_consumed_marker(value_slot);
            self.mark_consumed(value_slot);
        } else {
            self.emit_release(value_slot)?;
        }
        let some_label = self.b().new_label("L_MAP_INSERT_SOME");
        let none_label = self.b().new_label("L_MAP_INSERT_NONE");
        let end_label = self.b().new_label("L_MAP_INSERT_END");
        self.b().emit_branch(replaced, &some_label, &none_label);
        let branch_locals_len = self.state.frame.locals.len();
        let pre_released = clone_released(&self.state);
        self.b().emit_label(&some_label);
        crate::release_emit::emit_branch_release(&mut self.state, replaced);
        if plan.value_by_slot {
            let old_loaded = self.b().new_tmp();
            self.emit_load(old_loaded, old_val, 0, PrimType::U64);
            // The replaced entry's heap cell is freed now that
            // its value has been copied out for the Option.
            self.ensure_method_std_deps("sa_std/alloc/vec.sa", &["sa_mem_free"])?;
            let old_name = self.b().symbol_name(old_val).to_string();
            self.emit_method_call_body(None, format!("@sa_mem_free(^{old_name})"))?;
            let option_name = self.b().symbol_name(option_reg).to_string();
            let loaded_name = self.b().symbol_name(old_loaded).to_string();
            self.emit_method_macro_fragment(
                "sa_std/core/option.sa",
                "OPTION_NEW_SOME",
                &[option_name, loaded_name],
            )?;
            self.emit_release(old_loaded)?;
        } else {
            let option_name = self.b().symbol_name(option_reg).to_string();
            let old_name = self.b().symbol_name(old_val).to_string();
            self.emit_method_macro_fragment(
                "sa_std/core/option.sa",
                "OPTION_NEW_SOME",
                &[option_name, old_name],
            )?;
            crate::release_emit::emit_branch_release(&mut self.state, old_val);
        }
        self.b().emit_jmp(&end_label);
        let then_released = clone_released(&self.state);
        self.state.frame.pop_locals_to(branch_locals_len);
        restore_released(&mut self.state, &pre_released);
        self.b().emit_label(&none_label);
        crate::release_emit::emit_branch_release(&mut self.state, replaced);
        crate::release_emit::emit_branch_release(&mut self.state, old_val);
        let option_name = self.b().symbol_name(option_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/core/option.sa",
            "OPTION_NEW_NONE",
            &[option_name],
        )?;
        self.b().emit_jmp(&end_label);
        let else_released = clone_released(&self.state);
        self.state.frame.pop_locals_to(branch_locals_len);
        restore_released(&mut self.state, &pre_released);
        set_merge_released(
            &mut self.state,
            false,
            &then_released,
            false,
            &else_released,
            &pre_released,
        );
        self.b().emit_label(&end_label);
        self.release_non_local(recv_reg)?;
        // Checker-consumed key/value handles were adopted by
        // the map: retire them with a consumed marker instead
        // of leaving them for the scope-exit cleanup drop.
        if self.insert_arg_checker_consumed(&call.args[0], &call.args[2]) {
            self.emit_consumed_marker(value_reg);
            self.mark_consumed(value_reg);
        }
        if self.insert_arg_checker_consumed(&call.args[0], &key_arg) {
            self.emit_consumed_marker(key_reg);
            self.mark_consumed(key_reg);
        } else {
            self.release_expr_result_if_needed(&key_arg, key_reg)?;
        }
        Ok(Some(option_reg))
    }

    /// Zig `genMapGetCall` (21974): `MAP_TRY_GET` (found flag
    /// + slot pointer) folded into an Option. String-keyed
    /// maps route through the content-keyed `_STR` variant
    /// via the shared index plan (DEF-INSERT-GETMISS).
    fn gen_map_get_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.args.len() != 2 || call.func_name != "get" || call.associated_target.is_some() {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        if map_types(&receiver_ty, "HashMap").is_none() {
            return Ok(None);
        }
        let plan = match map_index_plan(&receiver_ty) {
            Some(plan) => plan,
            None => return Ok(None),
        };
        let recv_reg = self.gen_expr(&call.args[0])?;
        let key_arg = call.args[1].clone();
        let key_reg = self.gen_map_key_reg(&key_arg)?;
        self.ensure_method_std_deps(plan.import_path, &[plan.dep_symbol])?;
        let ok_reg = self.b().new_tmp();
        let payload_reg = self.b().new_tmp();
        let ok_name = self.b().symbol_name(ok_reg).to_string();
        let payload_name = self.b().symbol_name(payload_reg).to_string();
        let recv_name = self.b().symbol_name(recv_reg).to_string();
        let key_name = self.b().symbol_name(key_reg).to_string();
        self.emit_method_macro_fragment(
            plan.import_path,
            plan.macro_name,
            &[ok_name, payload_name, recv_name, key_name],
        )?;
        let option_reg = self.b().new_tmp();
        self.b().record_reg(option_reg);
        let some_label = self.b().new_label("L_MAP_GET_SOME");
        let none_label = self.b().new_label("L_MAP_GET_NONE");
        let end_label = self.b().new_label("L_MAP_GET_END");
        self.b().emit_branch(ok_reg, &some_label, &none_label);
        let branch_locals_len = self.state.frame.locals.len();
        let pre_released = clone_released(&self.state);
        self.b().emit_label(&some_label);
        crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
        let option_name = self.b().symbol_name(option_reg).to_string();
        let payload_name = self.b().symbol_name(payload_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/core/option.sa",
            "OPTION_NEW_SOME",
            &[option_name, payload_name],
        )?;
        crate::release_emit::emit_branch_release(&mut self.state, payload_reg);
        self.b().emit_jmp(&end_label);
        let then_released = clone_released(&self.state);
        self.state.frame.pop_locals_to(branch_locals_len);
        restore_released(&mut self.state, &pre_released);
        self.b().emit_label(&none_label);
        crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
        crate::release_emit::emit_branch_release(&mut self.state, payload_reg);
        let option_name = self.b().symbol_name(option_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/core/option.sa",
            "OPTION_NEW_NONE",
            &[option_name],
        )?;
        self.b().emit_jmp(&end_label);
        let else_released = clone_released(&self.state);
        self.state.frame.pop_locals_to(branch_locals_len);
        restore_released(&mut self.state, &pre_released);
        set_merge_released(
            &mut self.state,
            false,
            &then_released,
            false,
            &else_released,
            &pre_released,
        );
        self.b().emit_label(&end_label);
        self.release_non_local(recv_reg)?;
        self.release_expr_result_if_needed(&key_arg, key_reg)?;
        Ok(Some(option_reg))
    }

    /// Zig `genBTreeMapGetCall` (22080): `BTREE_MAP_TRY_GET`
    /// yields the value directly, so the SOME branch stages
    /// it through a stack slot first, exactly like SA.
    fn gen_btree_map_get_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.args.len() != 2 || call.func_name != "get" || call.associated_target.is_some() {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        if map_types(&receiver_ty, "BTreeMap").is_none() {
            return Ok(None);
        }
        let recv_reg = self.gen_expr(&call.args[0])?;
        let key_arg = call.args[1].clone();
        let key_reg = self.gen_map_key_reg(&key_arg)?;
        self.ensure_method_std_deps("sa_std/btree_map.sa", &["sa_btree_map_try_get"])?;
        let ok_reg = self.b().new_tmp();
        let val_reg = self.b().new_tmp();
        let ok_name = self.b().symbol_name(ok_reg).to_string();
        let val_name = self.b().symbol_name(val_reg).to_string();
        let recv_name = self.b().symbol_name(recv_reg).to_string();
        let key_name = self.b().symbol_name(key_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/btree_map.sa",
            "BTREE_MAP_TRY_GET",
            &[ok_name, val_name, recv_name, key_name],
        )?;
        let option_reg = self.b().new_tmp();
        self.b().record_reg(option_reg);
        let some_label = self.b().new_label("L_BTREE_GET_SOME");
        let none_label = self.b().new_label("L_BTREE_GET_NONE");
        let end_label = self.b().new_label("L_BTREE_GET_END");
        self.b().emit_branch(ok_reg, &some_label, &none_label);
        let branch_locals_len = self.state.frame.locals.len();
        let pre_released = clone_released(&self.state);
        self.b().emit_label(&some_label);
        crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
        let slot_reg = self.b().new_tmp();
        crate::release_prim::emit_stack_alloc(&mut self.state, slot_reg, 8);
        self.emit_store(slot_reg, 0, val_reg, PrimType::U64);
        let option_name = self.b().symbol_name(option_reg).to_string();
        let slot_name = self.b().symbol_name(slot_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/core/option.sa",
            "OPTION_NEW_SOME",
            &[option_name, slot_name],
        )?;
        crate::release_emit::emit_branch_release(&mut self.state, val_reg);
        self.b().emit_jmp(&end_label);
        let then_released = clone_released(&self.state);
        self.state.frame.pop_locals_to(branch_locals_len);
        restore_released(&mut self.state, &pre_released);
        self.b().emit_label(&none_label);
        crate::release_emit::emit_branch_release(&mut self.state, ok_reg);
        crate::release_emit::emit_branch_release(&mut self.state, val_reg);
        let option_name = self.b().symbol_name(option_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/core/option.sa",
            "OPTION_NEW_NONE",
            &[option_name],
        )?;
        self.b().emit_jmp(&end_label);
        let else_released = clone_released(&self.state);
        self.state.frame.pop_locals_to(branch_locals_len);
        restore_released(&mut self.state, &pre_released);
        set_merge_released(
            &mut self.state,
            false,
            &then_released,
            false,
            &else_released,
            &pre_released,
        );
        self.b().emit_label(&end_label);
        self.release_non_local(recv_reg)?;
        self.release_expr_result_if_needed(&key_arg, key_reg)?;
        Ok(Some(option_reg))
    }

    /// Zig `genMapContainsKeyCall` (22103): string keys probe
    /// by content through `@sa_map_contains_key_str` (direct
    /// call body); other keys use `MAP_CONTAINS_KEY`.
    fn gen_map_contains_key_call(&mut self, call: &CallExpr) -> Result<Option<u32>> {
        if call.args.len() != 2
            || call.func_name != "contains_key"
            || call.associated_target.is_some()
        {
            return Ok(None);
        }
        let receiver_ty = match self.check.expr_type(&call.args[0]).cloned() {
            Some(ty) => ty,
            None => return Ok(None),
        };
        let (key_ty, _) = match map_types(&receiver_ty, "HashMap") {
            Some(types) => types,
            None => return Ok(None),
        };
        let recv_reg = self.gen_expr(&call.args[0])?;
        let key_arg = call.args[1].clone();
        let key_reg = self.gen_map_key_reg(&key_arg)?;
        let result_reg = self.b().new_tmp();
        self.b().record_reg(result_reg);
        if is_string_type(&key_ty) {
            self.ensure_method_std_deps("sa_std/hashmap.sa", &["sa_map_contains_key_str"])?;
            let recv_name = self.b().symbol_name(recv_reg).to_string();
            let key_name = self.b().symbol_name(key_reg).to_string();
            self.emit_method_call_body(
                Some(result_reg),
                format!("@sa_map_contains_key_str(&{recv_name}, &{key_name})"),
            )?;
        } else {
            self.ensure_method_std_deps("sa_std/hashmap.sa", &["sa_map_contains_key"])?;
            let result_name = self.b().symbol_name(result_reg).to_string();
            let recv_name = self.b().symbol_name(recv_reg).to_string();
            let key_name = self.b().symbol_name(key_reg).to_string();
            self.emit_method_macro_fragment(
                "sa_std/hashmap.sa",
                "MAP_CONTAINS_KEY",
                &[result_name, recv_name, key_name],
            )?;
        }
        self.release_non_local(recv_reg)?;
        self.release_expr_result_if_needed(&key_arg, key_reg)?;
        Ok(Some(result_reg))
    }
}

/// Zig `MapInsertPlan` (lowering_rules.zig:4803).
struct MapInsertPlan {
    import_path: &'static str,
    macro_name: &'static str,
    dep_symbol: &'static str,
    value_by_slot: bool,
}

/// Zig `mapInsertPlan` (lowering_rules.zig:4803): owned
/// String keys are content-keyed (DEF-INSERT-GETMISS), so
/// they route through the `_STR` insert variant.
fn map_insert_plan(ty: &Type) -> Option<MapInsertPlan> {
    if let Some((key, _)) = map_types(ty, "HashMap") {
        if is_string_type(&key) {
            return Some(MapInsertPlan {
                import_path: "sa_std/hashmap.sa",
                macro_name: "MAP_INSERT_STR",
                dep_symbol: "sa_map_insert_str",
                value_by_slot: true,
            });
        }
        return Some(MapInsertPlan {
            import_path: "sa_std/hashmap.sa",
            macro_name: "MAP_INSERT",
            dep_symbol: "sa_map_insert",
            value_by_slot: true,
        });
    }
    if map_types(ty, "BTreeMap").is_some() {
        return Some(MapInsertPlan {
            import_path: "sa_std/btree_map.sa",
            macro_name: "BTREE_MAP_INSERT_OLD",
            dep_symbol: "sa_btree_map_insert_old",
            value_by_slot: false,
        });
    }
    None
}

/// Zig `MapIndexPlan` (lowering_rules.zig:4845), try_get_ptr
/// shape only (the BTreeMap shape is served by its own probe).
struct MapIndexPlan {
    import_path: &'static str,
    macro_name: &'static str,
    dep_symbol: &'static str,
}

/// Zig `mapIndexPlan` (lowering_rules.zig:4845), HashMap arm.
fn map_index_plan(ty: &Type) -> Option<MapIndexPlan> {
    let (key, _) = map_types(ty, "HashMap")?;
    if is_string_type(&key) {
        return Some(MapIndexPlan {
            import_path: "sa_std/hashmap.sa",
            macro_name: "MAP_TRY_GET_STR",
            dep_symbol: "sa_map_try_get_str",
        });
    }
    Some(MapIndexPlan {
        import_path: "sa_std/hashmap.sa",
        macro_name: "MAP_TRY_GET",
        dep_symbol: "sa_map_try_get",
    })
}

/// Zig `insertArgTransfersOwnership` (lowering_rules.zig:6844).
fn insert_arg_transfers_ownership(arg_ty: &Type, arg_is_copy_value_leaf: bool) -> bool {
    !arg_is_copy_value_leaf && !sla_lowering::is_borrow_like_type(arg_ty)
}
