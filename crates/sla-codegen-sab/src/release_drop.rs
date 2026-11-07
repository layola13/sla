//! E5 deep drop — the plan-driven deep-drop family, split
//! from release_locals.rs (E5) by the 700-line rule:
//! emitPlannedLocalDrop, emitDeepValueDrop,
//! emitSmartPointerDrop, the Vec/GrowVec container element
//! walkers, emitHashMapValueCellsFree and
//! emitStructFieldDrops (sab_codegen.zig lines 3311-3776
//! and 3673-3693 in the reference tree).
//!
//! The drop plan is the checker's (tc.planValueDrop /
//! tc.dropTaintCoversAt); it arrives through DropFacts
//! because the Rust CheckInfo surface carries no drop facts
//! yet. NoDropFacts is loud on the plan query, and
//! emit_planned_local_drop maps a plan error to false,
//! exactly Zig's catch-return-false. DropPlan mirrors
//! lowering_rules' union (lines 10231-10260) locally,
//! marked for re-homing into sla-lowering.

use sla_ast::Type;

use crate::error::{Error, Result};
use crate::ir::{OpKind, Operand, PrimType};
use crate::layout::{user_defined_generic_inner, vec_element_slot_size, vec_element_type};
use crate::release_emit::{emit_branch_release, emit_move, emit_release, MacroFragmentEmitter};
use crate::release_locals::{ensure_deps, emit_owned_buffer_free, DropEngine};
use crate::release_prim::{emit_load, emit_op, emit_ptr_add, emit_stack_alloc, emit_store};
use crate::state::CodegenState;
use crate::types::Local;


/// Zig `lowering_rules.DropPlan` (union, lines 10231–10260).
/// Zig stores child plans by pointer into checker-owned
/// storage; the port owns them (`Box`), shape-identical.
#[derive(Debug, Clone, PartialEq)]
pub enum DropPlan {
    None,
    Buffer,
    DeepVec(Box<DropPlan>),
    BufferGrowVec,
    DeepGrowVec(Box<DropPlan>),
    DeepStruct(Vec<FieldDropPlan>),
    ArcPtr(Box<DropPlan>),
    RcPtr(Box<DropPlan>),
    BoxPtr(Box<DropPlan>),
}

/// Zig `DropPlan.kind()` tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPlanKind {
    None,
    Buffer,
    DeepVec,
    BufferGrowVec,
    DeepGrowVec,
    DeepStruct,
    ArcPtr,
    RcPtr,
    BoxPtr,
}

impl DropPlan {
    pub fn kind(&self) -> DropPlanKind {
        match self {
            DropPlan::None => DropPlanKind::None,
            DropPlan::Buffer => DropPlanKind::Buffer,
            DropPlan::DeepVec(_) => DropPlanKind::DeepVec,
            DropPlan::BufferGrowVec => DropPlanKind::BufferGrowVec,
            DropPlan::DeepGrowVec(_) => DropPlanKind::DeepGrowVec,
            DropPlan::DeepStruct(_) => DropPlanKind::DeepStruct,
            DropPlan::ArcPtr(_) => DropPlanKind::ArcPtr,
            DropPlan::RcPtr(_) => DropPlanKind::RcPtr,
            DropPlan::BoxPtr(_) => DropPlanKind::BoxPtr,
        }
    }
}

/// Zig `lowering_rules.FieldDropPlan`.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDropPlan {
    pub name: String,
    pub plan: DropPlan,
}

/// The checker's drop facts as E5 consumes them
/// (`tc.planValueDrop` / `tc.dropTaintCoversAt`).
/// `node_key` is the `sla_lowering::node_key` of the current
/// drop node (Zig's `current_drop_node` pointer).
pub trait DropFacts {
    fn plan_value_drop(&self, ty: &Type) -> Result<DropPlan>;
    fn drop_taint_covers_at(&self, node_key: Option<usize>, place_key: &str) -> bool;
}

/// Loud default drop facts: no checker drop planning is
/// wired into this build.
pub struct NoDropFacts;

impl DropFacts for NoDropFacts {
    fn plan_value_drop(&self, _ty: &Type) -> Result<DropPlan> {
        Err(Error::UnsupportedSabDirectFeature(
            "tc.planValueDrop [group E5 release_locals; checker drop facts not wired]".to_string(),
        ))
    }
    fn drop_taint_covers_at(&self, _node_key: Option<usize>, _place_key: &str) -> bool {
        false
    }
}


impl<'c> DropEngine<'c> {
    /// Zig `emitPlannedLocalDrop`: plan-aware drop of a
    /// local's current value at an existing release decision
    /// point. `true` when a deep plan handled the drop;
    /// `false` leaves the caller on its shallow path.
    pub fn emit_planned_local_drop(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        local: &Local,
    ) -> Result<bool> {
        let ty = match &local.ty {
            Some(ty) => ty.clone(),
            None => return Ok(false),
        };
        // Zig: `tc.planValueDrop(ty) catch return false`.
        let plan = match self.drops.plan_value_drop(&ty) {
            Ok(plan) => plan,
            Err(_) => return Ok(false),
        };
        match plan.kind() {
            DropPlanKind::DeepVec
            | DropPlanKind::DeepGrowVec
            | DropPlanKind::DeepStruct
            | DropPlanKind::ArcPtr
            | DropPlanKind::RcPtr
            | DropPlanKind::BoxPtr => {}
            _ => return Ok(false),
        }
        if local.stack_ty.is_some() {
            let value = state.builder.new_tmp();
            emit_load(state, value, local.reg, 0, PrimType::Ptr);
            self.emit_deep_value_drop(state, frags, value, &ty, &plan, &local.name, false, true)?;
            emit_release(state, frags, local.reg)?;
            return Ok(true);
        }
        if local.is_stack_alloc {
            self.emit_deep_value_drop(state, frags, local.reg, &ty, &plan, &local.name, false, false)?;
            return Ok(true);
        }
        self.emit_deep_value_drop(state, frags, local.reg, &ty, &plan, &local.name, false, true)?;
        Ok(true)
    }

    /// Zig `emitDeepValueDrop`: plan-driven value drop.
    /// `may_alias` marks values loaded out of a place;
    /// `free_container` is false for inline stack storage.
    #[allow(clippy::too_many_arguments)]
    pub fn emit_deep_value_drop(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        value_reg: u32,
        ty: &Type,
        plan: &DropPlan,
        key: &str,
        may_alias: bool,
        free_container: bool,
    ) -> Result<()> {
        match plan {
            DropPlan::None => emit_release(state, frags, value_reg),
            DropPlan::Buffer | DropPlan::BufferGrowVec => {
                if may_alias && self.taint_covers(state, key) {
                    emit_move(state, value_reg);
                    return Ok(());
                }
                if free_container {
                    emit_owned_buffer_free(state, value_reg, Some(ty))
                } else {
                    emit_release(state, frags, value_reg)
                }
            }
            DropPlan::DeepVec(elem_plan) => {
                if may_alias && self.taint_covers(state, key) {
                    emit_move(state, value_reg);
                    return Ok(());
                }
                if !self.taint_covers(state, key) {
                    self.emit_vec_element_drops(state, frags, value_reg, ty, elem_plan, key)?;
                }
                if free_container {
                    emit_owned_buffer_free(state, value_reg, Some(ty))
                } else {
                    emit_release(state, frags, value_reg)
                }
            }
            DropPlan::DeepGrowVec(elem_plan) => {
                if may_alias && self.taint_covers(state, key) {
                    emit_move(state, value_reg);
                    return Ok(());
                }
                if !self.taint_covers(state, key) {
                    self.emit_growvec_element_drops(state, frags, value_reg, ty, elem_plan, key)?;
                }
                if free_container {
                    emit_owned_buffer_free(state, value_reg, Some(ty))
                } else {
                    emit_release(state, frags, value_reg)
                }
            }
            DropPlan::DeepStruct(fields) => {
                if may_alias && self.taint_covers(state, key) {
                    emit_move(state, value_reg);
                    return Ok(());
                }
                let fields = fields.clone();
                self.emit_struct_field_drops(state, frags, value_reg, ty, &fields, key)?;
                emit_release(state, frags, value_reg)
            }
            DropPlan::ArcPtr(_) | DropPlan::RcPtr(_) | DropPlan::BoxPtr(_) => {
                self.emit_smart_pointer_drop(state, frags, value_reg, ty, plan, key, may_alias)
            }
        }
    }

    /// Zig `emitVecElementDrops`: VecAbi offsets directly —
    /// never a layout lookup (the SAB-D14 history: the old
    /// `else |_| 8` fallback substituted cap's offset for
    /// len's and dropped uninitialised slots, SIGSEGV).
    pub fn emit_vec_element_drops(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        vec_reg: u32,
        vec_ty: &Type,
        elem_plan: &DropPlan,
        key: &str,
    ) -> Result<()> {
        let elem_ty = match vec_element_type(vec_ty) {
            Some(t) => t,
            None => return Ok(()),
        };
        self.emit_container_element_drops(
            state, frags, vec_reg, &elem_ty, elem_plan, key,
            VEC_ABI_PTR_OFFSET, VEC_ABI_LEN_OFFSET,
        )
    }

    /// Zig `emitGrowVecElementDrops`: GrowVecAbi offsets.
    pub fn emit_growvec_element_drops(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        vec_reg: u32,
        vec_ty: &Type,
        elem_plan: &DropPlan,
        key: &str,
    ) -> Result<()> {
        let elem_ty = match user_defined_generic_inner(vec_ty, "GrowVec") {
            Some(t) => t,
            None => return Ok(()),
        };
        self.emit_container_element_drops(
            state, frags, vec_reg, &elem_ty, elem_plan, key,
            GROWVEC_ABI_PTR_OFFSET, GROWVEC_ABI_LEN_OFFSET,
        )
    }

    /// Zig `emitContainerElementDrops`: the [0, len) walker.
    #[allow(clippy::too_many_arguments)]
    pub fn emit_container_element_drops(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        vec_reg: u32,
        elem_ty: &Type,
        elem_plan: &DropPlan,
        key: &str,
        ptr_off: usize,
        len_off: usize,
    ) -> Result<()> {
        let buf_reg = state.builder.new_tmp();
        emit_load(state, buf_reg, vec_reg, ptr_off, PrimType::Ptr);
        let len_reg = state.builder.new_tmp();
        emit_load(state, len_reg, vec_reg, len_off, PrimType::U64);
        let idx_slot = state.builder.new_tmp();
        emit_stack_alloc(state, idx_slot, 8);
        let zero_reg = state.builder.new_tmp();
        state.builder.emit_assign_imm(zero_reg, 0);
        emit_store(state, idx_slot, 0, zero_reg, PrimType::U64);
        emit_release(state, frags, zero_reg)?;
        let head_label = state.builder.new_label("L_DD_HEAD");
        let body_label = state.builder.new_label("L_DD_BODY");
        let done_label = state.builder.new_label("L_DD_DONE");
        state.builder.emit_jmp(&head_label);
        state.builder.emit_label(&head_label);
        let i_reg = state.builder.new_tmp();
        emit_load(state, i_reg, idx_slot, 0, PrimType::U64);
        let cond_reg = state.builder.new_tmp();
        emit_op(state, cond_reg, OpKind::Ult, Operand::Reg(i_reg), Operand::Reg(len_reg));
        state.builder.emit_branch(cond_reg, &body_label, &done_label);
        state.builder.emit_label(&body_label);
        let off_reg = state.builder.new_tmp();
        emit_op(
            state,
            off_reg,
            OpKind::Mul,
            Operand::Reg(i_reg),
            Operand::ImmI64(vec_element_slot_size(elem_ty) as i64),
        );
        let elem_addr = state.builder.new_tmp();
        emit_ptr_add(state, elem_addr, buf_reg, Operand::Reg(off_reg));
        emit_release(state, frags, off_reg)?;
        // Element slots hold the element VALUE (a pointer for
        // aggregate elements): load it out, then drop through
        // the canonical deep-value path.
        let elem_reg = state.builder.new_tmp();
        emit_load(state, elem_reg, elem_addr, 0, PrimType::Ptr);
        self.emit_deep_value_drop(state, frags, elem_reg, elem_ty, elem_plan, key, false, true)?;
        emit_release(state, frags, elem_addr)?;
        let next_reg = state.builder.new_tmp();
        emit_op(state, next_reg, OpKind::Add, Operand::Reg(i_reg), Operand::ImmI64(1));
        emit_store(state, idx_slot, 0, next_reg, PrimType::U64);
        emit_release(state, frags, next_reg)?;
        emit_branch_release(state, i_reg);
        emit_branch_release(state, cond_reg);
        state.builder.emit_jmp(&head_label);
        state.builder.emit_label(&done_label);
        emit_release(state, frags, buf_reg)?;
        emit_release(state, frags, len_reg)?;
        emit_release(state, frags, idx_slot)?;
        Ok(())
    }

    /// Zig `emitHashMapValueCellsFree` (DEF-HASHMAP-XTALK
    /// A-form): walk the table's slots (HashMap layout:
    /// slots+0, cap+8; HashMapSlot: hash+0, key+8, value+16,
    /// state+24, size 32) and free each FILLED slot's cell.
    pub fn emit_hashmap_value_cells_free(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        map_reg: u32,
    ) -> Result<()> {
        let slots_reg = state.builder.new_tmp();
        emit_load(state, slots_reg, map_reg, 0, PrimType::Ptr);
        let cap_reg = state.builder.new_tmp();
        emit_load(state, cap_reg, map_reg, 8, PrimType::U64);
        let idx_slot = state.builder.new_tmp();
        emit_stack_alloc(state, idx_slot, 8);
        let zero_reg = state.builder.new_tmp();
        state.builder.emit_assign_imm(zero_reg, 0);
        emit_store(state, idx_slot, 0, zero_reg, PrimType::U64);
        emit_release(state, frags, zero_reg)?;
        let head_label = state.builder.new_label("L_MAP_CELL_FREE_HEAD");
        let body_label = state.builder.new_label("L_MAP_CELL_FREE_BODY");
        let cell_label = state.builder.new_label("L_MAP_CELL_FREE_CELL");
        let skip_label = state.builder.new_label("L_MAP_CELL_FREE_SKIP");
        let merge_label = state.builder.new_label("L_MAP_CELL_FREE_MERGE");
        let done_label = state.builder.new_label("L_MAP_CELL_FREE_DONE");
        state.builder.emit_jmp(&head_label);
        state.builder.emit_label(&head_label);
        let i_reg = state.builder.new_tmp();
        emit_load(state, i_reg, idx_slot, 0, PrimType::U64);
        let cond_reg = state.builder.new_tmp();
        emit_op(state, cond_reg, OpKind::Ult, Operand::Reg(i_reg), Operand::Reg(cap_reg));
        state.builder.emit_branch(cond_reg, &body_label, &done_label);
        state.builder.emit_label(&body_label);
        let off_reg = state.builder.new_tmp();
        emit_op(state, off_reg, OpKind::Mul, Operand::Reg(i_reg), Operand::ImmI64(32));
        let slot_reg = state.builder.new_tmp();
        emit_ptr_add(state, slot_reg, slots_reg, Operand::Reg(off_reg));
        emit_release(state, frags, off_reg)?;
        let state_reg = state.builder.new_tmp();
        emit_load(state, state_reg, slot_reg, 24, PrimType::U8);
        let filled_reg = state.builder.new_tmp();
        emit_op(state, filled_reg, OpKind::Eq, Operand::Reg(state_reg), Operand::ImmI64(1));
        state.builder.emit_branch(filled_reg, &cell_label, &skip_label);
        state.builder.emit_label(&cell_label);
        emit_branch_release(state, filled_reg);
        let cell_reg = state.builder.new_tmp();
        emit_load(state, cell_reg, slot_reg, 16, PrimType::Ptr);
        ensure_deps(state, "sa_std/alloc/vec.sa", &["sa_mem_free"])?;
        let cell_name = state.builder.symbol_name(cell_reg).to_string();
        state.builder.emit_call(None, &format!("@sa_mem_free(^{cell_name})"));
        state.builder.emit_jmp(&merge_label);
        state.builder.emit_label(&skip_label);
        emit_branch_release(state, filled_reg);
        state.builder.emit_jmp(&merge_label);
        state.builder.emit_label(&merge_label);
        emit_release(state, frags, state_reg)?;
        emit_release(state, frags, slot_reg)?;
        let next_reg = state.builder.new_tmp();
        emit_op(state, next_reg, OpKind::Add, Operand::Reg(i_reg), Operand::ImmI64(1));
        emit_store(state, idx_slot, 0, next_reg, PrimType::U64);
        emit_release(state, frags, next_reg)?;
        emit_branch_release(state, i_reg);
        emit_branch_release(state, cond_reg);
        state.builder.emit_jmp(&head_label);
        state.builder.emit_label(&done_label);
        emit_release(state, frags, slots_reg)?;
        emit_release(state, frags, cap_reg)?;
        emit_release(state, frags, idx_slot)?;
        Ok(())
    }

    /// Zig `emitStructFieldDrops`.
    pub fn emit_struct_field_drops(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        struct_reg: u32,
        struct_ty: &Type,
        fields: &[FieldDropPlan],
        key: &str,
    ) -> Result<()> {
        let decl = match self.layout.struct_decl_for_type(struct_ty) {
            Some(d) => d,
            None => return Ok(()),
        };
        for field in fields {
            if field.plan.kind() == DropPlanKind::None {
                continue;
            }
            // Zig: `fieldLayout(...) catch continue` — see the
            // module docs (SAB-D103): reproduced skip.
            let layout = match self.layout.field_layout(struct_ty, &field.name) {
                Ok(l) => l,
                Err(_) => continue,
            };
            let fty = decl
                .fields
                .iter()
                .find(|f| f.name == field.name)
                .map(|f| (*f.ty).clone());
            let fty = match fty {
                Some(t) => t,
                None => continue,
            };
            let child_key = format!("{key}.{}", field.name);
            let val_reg = state.builder.new_tmp();
            emit_load(state, val_reg, struct_reg, layout.offset, layout.ty);
            self.emit_deep_value_drop(
                state, frags, val_reg, &fty, &field.plan, &child_key, true, true,
            )?;
        }
        Ok(())
    }

    /// Zig `emitSmartPointerDrop` (DEF-ARC-DROP F1): the
    /// sequence is emitted from primitives in exactly the
    /// std macros' shape — ATOMIC_U64_FETCH_SUB fragments for
    /// Arc counters, load/sub/store for Rc, explicit payload
    /// recursion, and the @sa_mem_free call for the box.
    /// Path-sensitive releases use raw branch releases: every
    /// control-flow path must dispose of the branch condition
    /// and of the handle exactly once (Zig comment).
    pub fn emit_smart_pointer_drop(
        &mut self,
        state: &mut CodegenState,
        frags: &mut dyn MacroFragmentEmitter,
        value_reg: u32,
        ty: &Type,
        plan: &DropPlan,
        key: &str,
        may_alias: bool,
    ) -> Result<()> {
        if may_alias && self.taint_covers(state, key) {
            emit_move(state, value_reg);
            return Ok(());
        }
        let (payload_ty, payload_plan) = match plan {
            DropPlan::ArcPtr(p) => (user_defined_generic_inner(ty, "Arc"), (**p).clone()),
            DropPlan::RcPtr(p) => (user_defined_generic_inner(ty, "Rc"), (**p).clone()),
            DropPlan::BoxPtr(p) => (user_defined_generic_inner(ty, "Box"), (**p).clone()),
            _ => (None, DropPlan::None),
        };
        let payload_ty = match payload_ty {
            Some(t) => t,
            None => return emit_release(state, frags, value_reg),
        };
        let value_name = state.builder.symbol_name(value_reg).to_string();
        if plan.kind() == DropPlanKind::BoxPtr {
            let data_reg = state.builder.new_tmp();
            emit_load(state, data_reg, value_reg, 0, PrimType::Ptr);
            self.emit_deep_value_drop(state, frags, data_reg, &payload_ty, &payload_plan, key, false, true)?;
            ensure_deps(state, "sa_std/alloc/vec.sa", &["sa_mem_free"])?;
            state.builder.emit_call(None, &format!("@sa_mem_free(^{value_name})"));
            crate::release_prim::mark_released(state, value_reg);
            return Ok(());
        }
        let is_arc = plan.kind() == DropPlanKind::ArcPtr;
        let clean_label = state.builder.new_label("L_SMART_CLEAN");
        let free_label = state.builder.new_label("L_SMART_FREE");
        let after_s_label = state.builder.new_label("L_SMART_AFTER_S");
        let after_w_label = state.builder.new_label("L_SMART_AFTER_W");
        let end_label = state.builder.new_label("L_SMART_END");
        let cond_reg;
        if is_arc {
            let old_reg = state.builder.new_tmp();
            crate::locals::record_reg(state, old_reg);
            let old_name = state.builder.symbol_name(old_reg).to_string();
            frags.emit_std_macro_fragment_with_literal_args(
                "sa_std/sync/atomic.sa",
                "ATOMIC_U64_FETCH_SUB",
                &[old_name, value_name.clone(), "1".to_string(), "acq_rel".to_string()],
                &[false, false, true, true],
            )?;
            cond_reg = state.builder.new_tmp();
            emit_op(state, cond_reg, OpKind::Eq, Operand::Reg(old_reg), Operand::ImmI64(1));
            emit_release(state, frags, old_reg)?;
            state.builder.emit_branch(cond_reg, &clean_label, &after_s_label);
        } else {
            let old_reg = state.builder.new_tmp();
            emit_load(state, old_reg, value_reg, 0, PrimType::U64);
            let new_reg = state.builder.new_tmp();
            emit_op(state, new_reg, OpKind::Sub, Operand::Reg(old_reg), Operand::ImmI64(1));
            emit_store(state, value_reg, 0, new_reg, PrimType::U64);
            emit_release(state, frags, new_reg)?;
            cond_reg = state.builder.new_tmp();
            emit_op(state, cond_reg, OpKind::Eq, Operand::Reg(old_reg), Operand::ImmI64(1));
            emit_release(state, frags, old_reg)?;
            state.builder.emit_branch(cond_reg, &clean_label, &after_s_label);
        }
        state.builder.emit_label(&after_s_label);
        emit_branch_release(state, cond_reg);
        crate::release_emit::emit_branch_move(state, value_reg);
        state.builder.emit_jmp(&end_label);
        state.builder.emit_label(&clean_label);
        emit_branch_release(state, cond_reg);
        let data_reg = state.builder.new_tmp();
        emit_load(state, data_reg, value_reg, 16, PrimType::Ptr);
        self.emit_deep_value_drop(state, frags, data_reg, &payload_ty, &payload_plan, key, false, true)?;
        let condw_reg;
        if is_arc {
            let weak_cell = state.builder.new_tmp();
            emit_ptr_add(state, weak_cell, value_reg, Operand::ImmI64(8));
            let oldw_reg = state.builder.new_tmp();
            crate::locals::record_reg(state, oldw_reg);
            let oldw_name = state.builder.symbol_name(oldw_reg).to_string();
            let weak_name = state.builder.symbol_name(weak_cell).to_string();
            frags.emit_std_macro_fragment_with_literal_args(
                "sa_std/sync/atomic.sa",
                "ATOMIC_U64_FETCH_SUB",
                &[oldw_name, weak_name, "1".to_string(), "acq_rel".to_string()],
                &[false, false, true, true],
            )?;
            emit_release(state, frags, weak_cell)?;
            condw_reg = state.builder.new_tmp();
            emit_op(state, condw_reg, OpKind::Eq, Operand::Reg(oldw_reg), Operand::ImmI64(1));
            emit_release(state, frags, oldw_reg)?;
            state.builder.emit_branch(condw_reg, &free_label, &after_w_label);
        } else {
            let oldw_reg = state.builder.new_tmp();
            emit_load(state, oldw_reg, value_reg, 8, PrimType::U64);
            let neww_reg = state.builder.new_tmp();
            emit_op(state, neww_reg, OpKind::Sub, Operand::Reg(oldw_reg), Operand::ImmI64(1));
            emit_store(state, value_reg, 8, neww_reg, PrimType::U64);
            emit_release(state, frags, neww_reg)?;
            condw_reg = state.builder.new_tmp();
            emit_op(state, condw_reg, OpKind::Eq, Operand::Reg(oldw_reg), Operand::ImmI64(1));
            emit_release(state, frags, oldw_reg)?;
            state.builder.emit_branch(condw_reg, &free_label, &after_w_label);
        }
        state.builder.emit_label(&after_w_label);
        emit_branch_release(state, condw_reg);
        crate::release_emit::emit_branch_move(state, value_reg);
        state.builder.emit_jmp(&end_label);
        state.builder.emit_label(&free_label);
        emit_branch_release(state, condw_reg);
        ensure_deps(state, "sa_std/alloc/vec.sa", &["sa_mem_free"])?;
        state.builder.emit_call(None, &format!("@sa_mem_free(^{value_name})"));
        state.builder.emit_jmp(&end_label);
        state.builder.emit_label(&end_label);
        crate::release_prim::mark_released(state, value_reg);
        Ok(())
    }

}

/// Zig `VecAbi` (lowering_rules.zig:3366).
pub const VEC_ABI_PTR_OFFSET: usize = 0;
pub const VEC_ABI_LEN_OFFSET: usize = 16;
/// Zig `GrowVecAbi` (lowering_rules.zig:3377).
pub const GROWVEC_ABI_PTR_OFFSET: usize = 0;
pub const GROWVEC_ABI_LEN_OFFSET: usize = 8;