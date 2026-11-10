//! S6 — `genIndex`: the IndexExpr read slice. Zig `genIndex`
//! (sab_codegen.zig:17129) with `genIndexAddress` (11806–
//! 11903), `genMapIndex`, and `branchReleaseIfOwned`, split
//! out of `driver_gen.rs` for the 700-line discipline.
//! Semantics are ported arm-by-arm from the Zig source — no
//! local invention:
//!
//! - (1) ordinary address targets (fixed array / Slice):
//!   `genIndexAddress` + a load of the element's storage
//!   prim type. The array path itself is shared with
//!   `gen_assign` (`gen_index_address_array`); the Slice
//!   fat-pointer path and the `.index_address` surface path
//!   live here.
//! - (2) map plan (HashMap / BTreeMap): `genMapIndex` —
//!   try-get fragment, miss panics with code 404.
//! - (3) Zig's `genVecDirectIndex` is dead code in the
//!   current Zig head (its first line returns null unless
//!   the symbol table length equals `maxInt(usize)`,
//!   issue014), so it is deliberately NOT revived here:
//!   Vec reads take arm (5), exactly as in Zig today.
//! - (4) VecDeque: the `VEC_DEQUE_GET` fragment.
//! - (5) fallback: the `.index` std surface rule for the
//!   target's base type name (Vec / GrowVec / Slice reads).

use sla_ast::{IndexExpr, Literal, Node, Type};

use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::fragment::FragmentMachine;
use crate::fragment_imports::MachineSurfaceEmitter;
use crate::gen_assign::AddressSource;
use crate::gen_method_call::map_types;
use crate::ir::{OpKind, Operand, PrimType};
use crate::layout::{
    element_load_type, element_slot_size, first_generic_arg, storage_prim_type, type_base_name,
    type_size, user_defined_generic_inner,
};
use crate::state::RegState;
use crate::surface_query::{
    emit_std_surface_rule, find_std_surface_rule, PendingStdDeps, StdSurfaceValues,
};
use crate::types::{StdSurfaceRule, StdSurfaceRuleKind};

/// Zig `ordinaryIndexAddressTargetType`
/// (lowering_rules.zig:153): peel borrows/pointers; a fixed
/// array or a one-argument `Slice` qualifies, everything
/// else is null.
fn ordinary_index_address_target_type(ty: &Type) -> Option<Type> {
    let peeled = sla_lowering::peel_borrow_pointer_type(ty);
    match peeled {
        Type::Array(_) => Some(peeled.clone()),
        Type::UserDefined(ud) if ud.name == "Slice" && ud.generics.len() == 1 => {
            Some(peeled.clone())
        }
        _ => None,
    }
}

/// Zig `indexFixedArrayElementType` (lowering_rules.zig:169).
fn index_fixed_array_element_type(ty: &Type) -> Option<Type> {
    match sla_lowering::peel_borrow_pointer_type(ty) {
        Type::Array(arr) => Some((*arr.elem).clone()),
        _ => None,
    }
}

/// Zig `MapIndexPlan` (lowering_rules.zig:4628): how a hit
/// payload reaches the destination — HashMap yields a slot
/// pointer (load through it), BTreeMap yields the value
/// payload directly (cast from the u64 slot).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapIndexShape {
    TryGetPtr,
    TryGetValue,
}

#[derive(Debug, Clone, Copy)]
struct MapIndexPlan {
    macro_name: &'static str,
    dep_symbol: &'static str,
    import_path: &'static str,
    shape: MapIndexShape,
}

/// Zig `mapIndexPlan` (lowering_rules.zig:4628). Distinct
/// from `gen_map_call`'s reduced plan for the `get` probe:
/// that one keys a `_STR` variant for string keys, which
/// Zig's `genMapIndex` does not have.
fn map_index_plan(ty: &Type) -> Option<MapIndexPlan> {
    if map_types(ty, "HashMap").is_some() {
        return Some(MapIndexPlan {
            macro_name: "MAP_TRY_GET",
            dep_symbol: "sa_map_try_get",
            import_path: "sa_std/hashmap.sa",
            shape: MapIndexShape::TryGetPtr,
        });
    }
    if map_types(ty, "BTreeMap").is_some() {
        return Some(MapIndexPlan {
            macro_name: "BTREE_MAP_TRY_GET",
            dep_symbol: "sa_btree_map_try_get",
            import_path: "sa_std/btree_map.sa",
            shape: MapIndexShape::TryGetValue,
        });
    }
    None
}

impl<'c> Driver<'c> {
    /// Zig `genIndex` (sab_codegen.zig:17129), dispatched
    /// from `gen_expr`'s IndexExpr arm.
    pub(crate) fn gen_index(&mut self, idx: &IndexExpr) -> Result<u32> {
        let target_ty = self.check.expr_type(&idx.target).cloned().ok_or_else(|| {
            Error::MissingType("index target has no checker type".to_string())
        })?;
        // (1) ordinary address target: address + load.
        if let Some(ordinary) = ordinary_index_address_target_type(&target_ty) {
            let elem_ty = match &ordinary {
                Type::Array(arr) => (*arr.elem).clone(),
                _ => first_generic_arg(&ordinary).ok_or_else(|| {
                    unsupported(
                        "ordinary index target without an element type",
                        "G gen_index",
                        "17129",
                    )
                })?,
            };
            let source = self.gen_index_address(idx)?;
            let dst = self.b().new_tmp();
            self.b().record_reg(dst);
            self.emit_load(dst, source.reg, 0, storage_prim_type(&elem_ty)?);
            self.release_address_source(&source)?;
            return Ok(dst);
        }
        // (2) map plan.
        if let Some(plan) = map_index_plan(&target_ty) {
            let container = match plan.shape {
                MapIndexShape::TryGetPtr => "HashMap",
                MapIndexShape::TryGetValue => "BTreeMap",
            };
            let (_, value_ty) = map_types(&target_ty, container).ok_or_else(|| {
                unsupported(
                    "map index target without key/value types",
                    "G gen_index",
                    "17129",
                )
            })?;
            return self.gen_map_index(idx, &plan, &value_ty);
        }
        // (4) VecDeque fragment arm.
        if let Some(elem_ty) = user_defined_generic_inner(&target_ty, "VecDeque") {
            return self.gen_vec_deque_index(idx, &elem_ty);
        }
        // (5) `.index` surface rule.
        self.gen_surface_index(idx, &target_ty)
    }

    /// Zig `genIndexAddress` (sab_codegen.zig:11806–11903):
    /// ordinary targets take the array / Slice paths, any
    /// other generic target takes the `.index_address`
    /// surface rule.
    pub(crate) fn gen_index_address(&mut self, idx: &IndexExpr) -> Result<AddressSource> {
        let target_ty = self.check.expr_type(&idx.target).cloned().ok_or_else(|| {
            Error::MissingType("index target has no checker type".to_string())
        })?;
        match ordinary_index_address_target_type(&target_ty) {
            Some(Type::Array(arr)) => self.gen_index_address_array(idx, &arr),
            Some(slice_ty) => self.gen_slice_index_address(idx, &slice_ty),
            _ => self.gen_surface_index_address(idx, &target_ty),
        }
    }

    /// Zig `genIndexAddress` Slice path (11811–11844): the
    /// slice value is a {ptr, len} pair; load the base
    /// pointer at `SliceAbi.ptr_offset` (0) and scale the
    /// index by the element size.
    fn gen_slice_index_address(
        &mut self,
        idx: &IndexExpr,
        slice_ty: &Type,
    ) -> Result<AddressSource> {
        let elem_ty = first_generic_arg(slice_ty).ok_or_else(|| {
            unsupported(
                "slice index target without a generic argument",
                "G gen_index",
                "11811",
            )
        })?;
        let elem_size = type_size(&elem_ty);
        let target_reg = self.gen_expr(&idx.target)?;
        let target_is_local = self.is_local_reg(target_reg);
        let base_ptr = self.b().new_tmp();
        self.b().record_reg(base_ptr);
        self.emit_load(base_ptr, target_reg, 0, PrimType::Ptr);
        if let Node::Literal(Literal::IntVal(raw)) = &*idx.index {
            if *raw < 0 {
                return Err(unsupported("negative slice index", "G gen_index", "11820"));
            }
            let offset = elem_size * (*raw as usize);
            let mut source = self.address_with_offset(base_ptr, offset);
            if !target_is_local {
                // Zig releases the loaded base when the target
                // was a moved non-local, and the target itself
                // when non-local; under the port's local/temp
                // dichotomy both ride on the same condition.
                let mut release_regs = Vec::new();
                if offset != 0 {
                    release_regs.push(base_ptr);
                }
                release_regs.push(target_reg);
                source.release_regs = release_regs;
            }
            return Ok(source);
        }
        let index_reg = self.gen_expr(&idx.index)?;
        let offset = self.b().new_tmp();
        self.b().record_reg(offset);
        crate::release_prim::emit_op(
            &mut self.state,
            offset,
            OpKind::Mul,
            Operand::Reg(index_reg),
            Operand::ImmI64(elem_size as i64),
        );
        let elem_ptr = self.b().new_tmp();
        self.b().record_reg(elem_ptr);
        crate::release_prim::emit_ptr_add(
            &mut self.state,
            elem_ptr,
            base_ptr,
            Operand::Reg(offset),
        );
        self.emit_release(offset)?;
        if !target_is_local {
            self.emit_release(base_ptr)?;
        }
        if !self.is_local_reg(index_reg) {
            self.emit_release(index_reg)?;
        }
        Ok(AddressSource {
            reg: elem_ptr,
            release_regs: if target_is_local {
                Vec::new()
            } else {
                vec![target_reg]
            },
        })
    }

    /// Zig `genIndexAddress` `.index_address` path
    /// (11846–11863): the surface rule yields the element
    /// address directly.
    fn gen_surface_index_address(
        &mut self,
        idx: &IndexExpr,
        target_ty: &Type,
    ) -> Result<AddressSource> {
        if first_generic_arg(target_ty).is_none() {
            return Err(unsupported(
                "index_address target without a generic argument",
                "G gen_index",
                "11846",
            ));
        }
        let base = type_base_name(target_ty).ok_or_else(|| {
            unsupported(
                "index_address target without a base type name",
                "G gen_index",
                "11846",
            )
        })?;
        let rule = find_std_surface_rule(
            &self.state.program.std_surface_rules,
            StdSurfaceRuleKind::IndexAddress,
            &base,
            None,
        )
        .ok_or_else(|| {
            unsupported(
                &format!("index_address surface rule for `{base}`"),
                "G gen_index",
                "11850",
            )
        })?
        .clone();
        let target_reg = self.gen_expr(&idx.target)?;
        let index_reg = self.gen_expr(&idx.index)?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let values = StdSurfaceValues {
            out: Some(dst),
            receiver: Some(target_reg),
            index: Some(index_reg),
            elem_size: element_slot_size(target_ty),
            elem_ty: Some(element_load_type(target_ty)?),
            ..StdSurfaceValues::default()
        };
        self.emit_index_rule(&rule, &values)?;
        self.release_non_local(target_reg)?;
        self.release_non_local(index_reg)?;
        Ok(AddressSource {
            reg: dst,
            release_regs: Vec::new(),
        })
    }

    /// Zig `genMapIndex` (sab_codegen.zig, the map arm of
    /// `genIndex`): try-get fragment; on a miss the owned
    /// branch registers are released and the read panics
    /// with code 404. No Option fold — the shapes follow
    /// `gen_method_surface`'s fallible-method precedent.
    fn gen_map_index(
        &mut self,
        idx: &IndexExpr,
        plan: &MapIndexPlan,
        value_ty: &Type,
    ) -> Result<u32> {
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let map_reg = self.gen_expr(&idx.target)?;
        let key_reg = self.gen_map_key_reg(&idx.index)?;
        self.ensure_method_std_deps(plan.import_path, &[plan.dep_symbol])?;
        let ok_reg = self.b().new_tmp();
        let payload_reg = self.b().new_tmp();
        let ok_name = self.b().symbol_name(ok_reg).to_string();
        let payload_name = self.b().symbol_name(payload_reg).to_string();
        let map_name = self.b().symbol_name(map_reg).to_string();
        let key_name = self.b().symbol_name(key_reg).to_string();
        self.emit_method_macro_fragment(
            plan.import_path,
            plan.macro_name,
            &[ok_name, payload_name, map_name, key_name],
        )?;
        let hit_label = self.b().new_label("L_MAP_INDEX_HIT");
        let miss_label = self.b().new_label("L_MAP_INDEX_MISS");
        self.b().emit_branch(ok_reg, &hit_label, &miss_label);
        self.b().emit_label(&miss_label);
        self.branch_release_if_owned(ok_reg);
        self.branch_release_if_owned(payload_reg);
        self.branch_release_if_owned(map_reg);
        self.branch_release_if_owned(key_reg);
        self.b().emit_panic_code(404);
        self.b().emit_label(&hit_label);
        self.branch_release_if_owned(ok_reg);
        let load_ty = element_load_type(value_ty)?;
        match plan.shape {
            MapIndexShape::TryGetValue => {
                self.b().emit_op(
                    dst,
                    crate::gen_struct::op_kind_for_cast(PrimType::U64, load_ty)?,
                    Operand::Reg(payload_reg),
                    Operand::Ty(load_ty as u32),
                );
            }
            MapIndexShape::TryGetPtr => {
                self.emit_load(dst, payload_reg, 0, load_ty);
            }
        }
        self.branch_release_if_owned(payload_reg);
        self.release_non_local(map_reg)?;
        self.release_expr_result_if_needed(&idx.index, key_reg)?;
        Ok(dst)
    }

    /// Zig `genIndex` VecDeque arm: the `VEC_DEQUE_GET`
    /// fragment (`sa_std/vec_deque.sa`).
    fn gen_vec_deque_index(&mut self, idx: &IndexExpr, _elem_ty: &Type) -> Result<u32> {
        let target_reg = self.gen_expr(&idx.target)?;
        let index_reg = self.gen_expr(&idx.index)?;
        self.ensure_method_std_deps("sa_std/vec_deque.sa", &["sa_vec_deque_get"])?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let dst_name = self.b().symbol_name(dst).to_string();
        let target_name = self.b().symbol_name(target_reg).to_string();
        let index_name = self.b().symbol_name(index_reg).to_string();
        self.emit_method_macro_fragment(
            "sa_std/vec_deque.sa",
            "VEC_DEQUE_GET",
            &[dst_name, target_name, index_name],
        )?;
        self.release_non_local(target_reg)?;
        self.release_expr_result_if_needed(&idx.index, index_reg)?;
        Ok(dst)
    }

    /// Zig `genIndex` fallback arm: the `.index` std
    /// surface rule for the target's base type name.
    fn gen_surface_index(&mut self, idx: &IndexExpr, target_ty: &Type) -> Result<u32> {
        let base = type_base_name(target_ty).ok_or_else(|| {
            unsupported(
                "index target without a base type name",
                "G gen_index",
                "17129",
            )
        })?;
        let rule = find_std_surface_rule(
            &self.state.program.std_surface_rules,
            StdSurfaceRuleKind::Index,
            &base,
            None,
        )
        .ok_or_else(|| {
            unsupported(
                &format!("index surface rule for `{base}`"),
                "G gen_index",
                "17129",
            )
        })?
        .clone();
        let target_reg = self.gen_expr(&idx.target)?;
        let index_reg = self.gen_expr(&idx.index)?;
        let dst = self.b().new_tmp();
        self.b().record_reg(dst);
        let values = StdSurfaceValues {
            out: Some(dst),
            receiver: Some(target_reg),
            index: Some(index_reg),
            elem_size: element_slot_size(target_ty),
            elem_ty: Some(element_load_type(target_ty)?),
            ..StdSurfaceValues::default()
        };
        self.emit_index_rule(&rule, &values)?;
        self.release_non_local(target_reg)?;
        self.release_non_local(index_reg)?;
        Ok(dst)
    }

    /// Zig `branchReleaseIfOwned`: release a register on a
    /// branch path unless it is a frame local, a stack
    /// allocation, or marked non-owning.
    fn branch_release_if_owned(&mut self, reg: u32) {
        if self.is_local_reg(reg) {
            return;
        }
        if self.state.frame.stack_alloc_emitted.contains(&reg) {
            return;
        }
        if matches!(self.state.frame.reg_state(reg), RegState::NonOwning) {
            return;
        }
        crate::release_emit::emit_branch_release(&mut self.state, reg);
    }

    /// Zig `emitStdSurfaceRule` (D4) with the production
    /// emitter; same queue/symbol plumbing as
    /// gen_assign's `emit_assign_rule` (per-module copy).
    fn emit_index_rule(&mut self, rule: &StdSurfaceRule, values: &StdSurfaceValues) -> Result<()> {
        let symbols: Vec<String> = self.b().symbols().to_vec();
        let mut included = std::mem::take(&mut self.state.program.included_imports);
        let mut pending = std::mem::take(&mut self.state.program.pending_std_deps);
        let result = {
            let mut emitter = MachineSurfaceEmitter {
                machine: FragmentMachine::new(&mut self.state),
                flattener: &mut *self.flattener,
            };
            let mut ensurer = PendingStdDeps::new(&mut included, &mut pending);
            emit_std_surface_rule(&mut emitter, &mut ensurer, rule, values, &symbols)
        };
        self.state.program.included_imports = included;
        self.state.program.pending_std_deps = pending;
        result
    }
}

#[cfg(test)]
mod tests;
