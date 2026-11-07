//! I-group emission primitives and stored-value release helpers.
//!
//! The B5 trunk (`emit.rs`) ports only the scalar primitives; the
//! aggregate/closure family needs the memory primitives Zig's
//! `Codegen` keeps beside them in sab_codegen.zig (current-head
//! line numbers, reference tree /content/sa_all/sa_plugin_sla @
//! 2cc12ea8): `emitAlloc` 4716, `emitStore` 4770, `emitStoreText`
//! 4779, `emitLoad` 4750, `emitPtrAdd` 4707, `emitMove` 4481,
//! `emitRelease` 4417, `emitConsumedMarker` 4788, `markConsumed`
//! 4539. They are implemented here, inside the I group's own
//! module, on top of the trunk's public `append_instruction`, so
//! the F-group-owned trunk file needs no I-group edits; the
//! instruction shapes are byte-identical to the Zig bodies
//! (operand order and tags included).
//!
//! The release helpers (`releaseExprResultIfNeeded` 4240,
//! `releaseStoredExprResultIfNeeded` 4259,
//! `releaseCopiedFieldValue` 4299, `releaseCopiedArrayFieldSource`
//! 4314, `markStoredValueMovedIfNeeded` 4543) are the E-group
//! surface the aggregate paths consume. Their Zig bodies consult
//! release state this port keeps in `FrameState::reg_states`
//! (SAB-D09): `Released` is Zig's `released_regs` membership,
//! `NonOwning` its `non_owning_regs`, `StackAlloc` its
//! `stack_alloc_regs`. `emitRelease`'s RefCell/mutex/rwlock guard
//! arms call F-group std fragments; a register carrying such a
//! payload stops loudly here instead of being silently released
//! the plain way (see REPORT wiring notes).

use sla_ast::{Node, Type};
use sla_lowering::{
    call_arg_needs_release, expr_result_needs_release, stored_value_moves_identifier,
};

use crate::driver::{unsupported, Driver};
use crate::error::Result;
use crate::ir::{InstKind, Instruction, Operand, PrimType};
use crate::state::RegState;

/// Zig `lowering_rules.isHeapHandleType` (lowering_rules.zig:1323).
pub(crate) fn is_heap_handle_type(ty: &Type) -> bool {
    matches!(ty, Type::UserDefined(_) | Type::Tuple(_) | Type::Array(_))
}

/// Zig `lowering_rules.typeIsPointerScalarValue`
/// (lowering_rules.zig:6589).
pub(crate) fn type_is_pointer_scalar_value(ty: &Type) -> bool {
    match ty {
        Type::Primitive(p) => *p == sla_ast::Primitive::RawPtr,
        Type::Pointer(_) => true,
        _ => false,
    }
}

/// Zig `ownedBufferDropType` (sab_codegen.zig:3268).
pub(crate) fn owned_buffer_drop_type(ty: Option<&Type>) -> bool {
    match ty {
        Some(Type::UserDefined(ud)) => {
            matches!(ud.name.as_str(), "String" | "Vec" | "GrowVec")
        }
        _ => false,
    }
}

impl<'c> Driver<'c> {
    fn push_inst(&mut self, inst: Instruction) {
        self.b().append_instruction(inst);
    }

    fn make(&mut self, kind: InstKind) -> Instruction {
        Instruction::new(kind, 0, self.b().instructions_len() as u32)
    }

    /// Zig `emitAlloc` (4716): `alloc dst, imm_u64(size)`.
    pub(crate) fn emit_alloc(&mut self, dst: u32, size: usize) {
        self.b().record_reg(dst);
        let mut inst = self.make(InstKind::Alloc);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::ImmU64(size as u64);
        self.push_inst(inst);
    }

    /// Zig `emitStore` (4770).
    pub(crate) fn emit_store(&mut self, base: u32, offset: usize, value: u32, ty: PrimType) {
        let mut inst = self.make(InstKind::Store);
        inst.operands[0] = Operand::Reg(base);
        inst.operands[1] = Operand::ImmU64(offset as u64);
        inst.operands[2] = Operand::Reg(value);
        inst.operands[3] = Operand::Ty(ty as u32);
        self.push_inst(inst);
    }

    /// Zig `emitStoreText` (4779): the value operand is raw text
    /// (the const-string symbol form `&SLA_STR_n`).
    pub(crate) fn emit_store_text(&mut self, base: u32, offset: usize, value: &str, ty: PrimType) {
        let mut inst = self.make(InstKind::Store);
        inst.operands[0] = Operand::Reg(base);
        inst.operands[1] = Operand::ImmU64(offset as u64);
        inst.operands[2] = Operand::Text(value.to_string());
        inst.operands[3] = Operand::Ty(ty as u32);
        self.push_inst(inst);
    }

    /// Zig `emitLoad` (4750).
    pub(crate) fn emit_load(&mut self, dst: u32, base: u32, offset: usize, ty: PrimType) {
        self.b().record_reg(dst);
        let mut inst = self.make(InstKind::Load);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::Reg(base);
        inst.operands[2] = Operand::ImmU64(offset as u64);
        inst.operands[3] = Operand::Ty(ty as u32);
        self.push_inst(inst);
    }

    /// Zig `emitPtrAdd` (4707) with an `imm_u64` offset operand.
    pub(crate) fn emit_ptr_add_imm(&mut self, dst: u32, base: u32, offset: usize) {
        self.b().record_reg(dst);
        let mut inst = self.make(InstKind::PtrAdd);
        inst.operands[0] = Operand::Reg(dst);
        inst.operands[1] = Operand::Reg(base);
        inst.operands[2] = Operand::ImmU64(offset as u64);
        self.push_inst(inst);
    }

    /// Zig `markConsumed` (4539): bookkeeping only, no instruction.
    pub(crate) fn mark_consumed(&mut self, reg: u32) {
        self.state.frame.reg_states.insert(reg, RegState::Consumed);
    }

    /// Zig `markNonOwningReg`: bookkeeping only.
    pub(crate) fn mark_non_owning(&mut self, reg: u32) {
        self.state.frame.reg_states.insert(reg, RegState::NonOwning);
    }

    /// Whether Zig's `released_regs` already contains `reg`. The
    /// port's `RegState::Released` and `RegState::Consumed` both
    /// record a register the Zig side would have put into
    /// `released_regs` (via `emitRelease`/`emitMove`/`markConsumed`).
    pub(crate) fn reg_released(&self, reg: u32) -> bool {
        matches!(
            self.state.frame.reg_states.get(&reg),
            Some(RegState::Released) | Some(RegState::Consumed)
        )
    }

    /// Zig `emitMove` (4481): `move_` + released bookkeeping,
    /// skipped when already released.
    pub(crate) fn emit_move(&mut self, reg: u32) {
        if self.reg_released(reg) {
            return;
        }
        let mut inst = self.make(InstKind::Move);
        inst.operands[0] = Operand::Reg(reg);
        self.push_inst(inst);
        self.state.frame.reg_states.insert(reg, RegState::Released);
    }

    /// Zig `emitRelease` (4417), the arms reachable from aggregate
    /// and closure paths: stack-alloc registers retire silently
    /// (bookkeeping only), non-owning registers retire with
    /// `move_`, everything else with `release`. Guard/RefCell
    /// payload registers belong to the F-group fragment releases
    /// and stop loudly.
    pub(crate) fn emit_release(&mut self, reg: u32) -> Result<()> {
        if self.reg_released(reg) {
            return Ok(());
        }
        if !self.state.frame.refcell_borrow_values.is_empty()
            && self.state.frame.refcell_borrow_values.contains_key(&reg)
        {
            return Err(unsupported(
                "release of a RefCell borrow register (fragment release)",
                "F fragments / E7 release_emit",
                "4420-4425",
            ));
        }
        if self.state.frame.mutex_guard_values.contains_key(&reg)
            || self.state.frame.rwlock_guard_values.contains_key(&reg)
        {
            return Err(unsupported(
                "release of a mutex/rwlock guard register (fragment release)",
                "F fragments / E7 release_emit",
                "4426-4431",
            ));
        }
        match self.state.frame.reg_state(reg) {
            RegState::StackAlloc => {
                self.state.frame.reg_states.insert(reg, RegState::Released);
                Ok(())
            }
            RegState::NonOwning => {
                self.emit_move(reg);
                Ok(())
            }
            _ => {
                let mut inst = self.make(InstKind::Release);
                inst.operands[0] = Operand::Reg(reg);
                self.push_inst(inst);
                self.state.frame.reg_states.insert(reg, RegState::Released);
                Ok(())
            }
        }
    }

    /// Zig `emitConsumedMarker` (4788): a `fence` instruction whose
    /// text operand is `^<symbol name>`.
    pub(crate) fn emit_consumed_marker(&mut self, value: u32) {
        let name = self.b().symbol_name(value).to_string();
        let mut inst = self.make(InstKind::Fence);
        inst.operands[0] = Operand::Text(format!("^{name}"));
        self.push_inst(inst);
    }

    /// Zig `isLocalReg` (4220).
    pub(crate) fn is_local_reg(&self, reg: u32) -> bool {
        self.state.frame.locals.iter().any(|local| local.reg == reg)
    }

    /// Zig `isParamReg` (4227): the *first* local with this register
    /// decides, as in Zig's linear scan.
    pub(crate) fn is_param_reg(&self, reg: u32) -> bool {
        for local in &self.state.frame.locals {
            if local.reg == reg {
                return local.is_param;
            }
        }
        false
    }

    /// Zig `stackLocal(name) != null`, over the port's stack-alloc
    /// locals (baseline paths create none; the check is kept so the
    /// release helpers below keep the Zig branch structure).
    pub(crate) fn has_stack_local(&self, name: &str) -> bool {
        self.state
            .frame
            .locals
            .iter()
            .rev()
            .any(|local| local.name == name && local.is_stack_alloc)
    }

    /// Zig `releaseExprResultIfNeeded` (4240). The owned-buffer arm
    /// calls `emitOwnedBufferFree` (std fragments, F group); in
    /// this port it stops loudly rather than degrading to a bare
    /// release (which would leak the buffer, per the Zig comment).
    pub(crate) fn release_expr_result_if_needed(&mut self, expr: &Node, reg: u32) -> Result<()> {
        if let Node::Identifier(name) = expr {
            if self.has_stack_local(name) {
                if !self.is_local_reg(reg) {
                    self.emit_release(reg)?;
                }
                return Ok(());
            }
        }
        if !expr_result_needs_release(expr) {
            return Ok(());
        }
        if !self.is_local_reg(reg) {
            let expr_ty = self.check.expr_type(expr).cloned();
            if owned_buffer_drop_type(expr_ty.as_ref()) {
                return Err(unsupported(
                    "release of an owned String/Vec/GrowVec temporary (owned-buffer free fragment)",
                    "F fragments / D surface",
                    "4248-4252",
                ));
            }
            self.emit_release(reg)?;
        }
        Ok(())
    }

    /// Zig `releaseStoredExprResultIfNeeded` (4259), including the
    /// DEF-ACL heap-handle transfer arm (consumed marker instead of
    /// a release).
    pub(crate) fn release_stored_expr_result_if_needed(
        &mut self,
        expr: &Node,
        reg: u32,
        stored_ty: &Type,
    ) -> Result<()> {
        if let Node::Identifier(name) = expr {
            if self.has_stack_local(name) {
                if !self.is_local_reg(reg) {
                    if is_heap_handle_type(stored_ty) {
                        self.emit_consumed_marker(reg);
                        self.mark_consumed(reg);
                        return Ok(());
                    }
                    if type_is_pointer_scalar_value(stored_ty) {
                        self.mark_non_owning(reg);
                    }
                    self.emit_release(reg)?;
                }
                return Ok(());
            }
        }
        if !expr_result_needs_release(expr) {
            return Ok(());
        }
        if !self.is_local_reg(reg) {
            if is_heap_handle_type(stored_ty) {
                self.emit_consumed_marker(reg);
                self.mark_consumed(reg);
                return Ok(());
            }
            if type_is_pointer_scalar_value(stored_ty) {
                self.mark_non_owning(reg);
            }
            self.emit_release(reg)?;
        }
        Ok(())
    }

    /// Zig `releaseCopiedFieldValue` (4299).
    pub(crate) fn release_copied_field_value(&mut self, copied: u32, field_ty: &Type) -> Result<()> {
        if is_heap_handle_type(field_ty) {
            self.emit_consumed_marker(copied);
            self.mark_consumed(copied);
            return Ok(());
        }
        self.emit_release(copied)
    }

    /// Zig `releaseCopiedArrayFieldSource` (4314): the pre-DEF-ACL
    /// body — no heap-handle transfer arm.
    pub(crate) fn release_copied_array_field_source(
        &mut self,
        expr: &Node,
        reg: u32,
        stored_ty: &Type,
    ) -> Result<()> {
        if let Node::Identifier(name) = expr {
            if self.has_stack_local(name) {
                if !self.is_local_reg(reg) {
                    if type_is_pointer_scalar_value(stored_ty) {
                        self.mark_non_owning(reg);
                    }
                    self.emit_release(reg)?;
                }
                return Ok(());
            }
        }
        if !expr_result_needs_release(expr) {
            return Ok(());
        }
        if !self.is_local_reg(reg) {
            if type_is_pointer_scalar_value(stored_ty) {
                self.mark_non_owning(reg);
            }
            self.emit_release(reg)?;
        }
        Ok(())
    }

    /// Zig `markStoredValueMovedIfNeeded` (4543).
    pub(crate) fn mark_stored_value_moved_if_needed(&mut self, value: &Node, value_ty: &Type) {
        let copy = self.check.type_is_copy_value(value_ty);
        if stored_value_moves_identifier(value, value_ty, copy).is_none() {
            return;
        }
        if let Node::Identifier(name) = value {
            if let Some(reg) = self.state.frame.local_reg(name) {
                self.mark_consumed(reg);
            }
        }
    }

    /// Zig `callArgNeedsRelease` re-export for field plans
    /// (update-source release flag).
    pub(crate) fn update_source_needs_release(expr: &Node) -> bool {
        call_arg_needs_release(expr)
    }
}

