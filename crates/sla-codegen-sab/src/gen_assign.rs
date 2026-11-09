//! B4 (continued) — `genAssign` for the non-identifier targets:
//! Zig `genAssign` (sab_codegen.zig:10335–10420) deref / field /
//! index arms, split out of `gen_stmt.rs` for the 700-line file
//! discipline. The identifier arm stays in `gen_stmt.rs` (it
//! predates this port); every other target kind keeps the loud
//! stop. Semantics are ported arm-by-arm from the Zig source —
//! no local invention:
//!
//! - deref (10336–10364): MutexGuard / RwLockWriteGuard targets
//!   store straight to the guard address; borrow / pointer
//!   targets store through, with the
//!   `planDerefAssignmentTargetLifecycle` release plan.
//! - field (10366–10382): `genFieldAddress` + store of the
//!   field's prim type; tuple targets take the numeric field
//!   index. Zig's arm releases the address register itself but
//!   not the address source's `release_regs` — mirrored as
//!   written.
//! - index (10384–10413): a non-array target goes through the
//!   `.index_assign` std surface rule for its base type name
//!   (Vec → `VEC_SET_TYPED`); an array target goes through
//!   `genIndexAddress` + store. Only the array path of
//!   `genIndexAddress` is ported here — the Slice /
//!   `.index_address` paths are unreachable from `genAssign`'s
//!   array guard (a Slice or generic target is not
//!   `Type::Array`, so it takes the surface arm first) and
//!   belong to the IndexExpr read slice.

use sla_ast::{ArrayType, AssignStmt, DerefExpr, FieldExpr, IndexExpr, Literal, Node, Type};

use crate::copy_facts::CopyFacts;
use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::fragment::FragmentMachine;
use crate::fragment_imports::MachineSurfaceEmitter;
use crate::ir::{OpKind, Operand};
use crate::layout::{
    array_element_layout, array_stride, element_load_type, element_slot_size, prim_type,
    storage_prim_type, tuple_field_layout, type_base_name, user_defined_generic_inner,
    LayoutAdapter,
};
use crate::surface_query::{
    emit_std_surface_rule, find_std_surface_rule, PendingStdDeps, StdSurfaceValues,
};
use crate::types::{StdSurfaceRule, StdSurfaceRuleKind};

/// Zig `AddressSource`: an address register plus the extra
/// registers a caller releases together with it
/// (`releaseAddressSource`).
struct AddressSource {
    reg: u32,
    release_regs: Vec<u32>,
}

/// Zig `DerefAssignmentTargetLifecycleAction`
/// (lowering_rules.zig:3529).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DerefAssignLifecycle {
    Keep,
    ReleaseValue,
    ReleaseBorrowHandle,
}

impl DerefAssignLifecycle {
    /// Zig `shouldRelease` (lowering_rules.zig:3534).
    fn should_release(self) -> bool {
        self != DerefAssignLifecycle::Keep
    }
}

/// Zig `planDerefAssignmentTargetLifecycle`
/// (lowering_rules.zig:3647).
fn plan_deref_assignment_target_lifecycle(
    target_is_temporary: bool,
    carries_refcell_borrow_handle: bool,
) -> DerefAssignLifecycle {
    if !target_is_temporary {
        return DerefAssignLifecycle::Keep;
    }
    if carries_refcell_borrow_handle {
        return DerefAssignLifecycle::ReleaseBorrowHandle;
    }
    DerefAssignLifecycle::ReleaseValue
}

impl<'c> Driver<'c> {
    /// Zig `genAssign` (10335–10420) for the deref / field /
    /// index target arms. `gen_stmt` dispatches here only for
    /// those target kinds.
    pub(crate) fn gen_assign(&mut self, assign: &AssignStmt) -> Result<()> {
        match &*assign.target {
            Node::DerefExpr(d) => self.gen_assign_deref(assign, d),
            Node::FieldExpr(f) => self.gen_assign_field(assign, f),
            Node::IndexExpr(i) => self.gen_assign_index(assign, i),
            _ => Err(unsupported(
                "assign to non-identifier target",
                "G gen_assign",
                "10335",
            )),
        }
    }

    /// Zig `genAssign` deref arm (10336–10364).
    fn gen_assign_deref(&mut self, assign: &AssignStmt, deref: &DerefExpr) -> Result<()> {
        let source_expr = &deref.expr;
        let source_ty = self.check.expr_type(source_expr).cloned().ok_or_else(|| {
            Error::MissingType("deref assign target has no checker type".to_string())
        })?;
        // Mirror SA-text: both MutexGuard and RwLockWriteGuard
        // deref targets store directly to the guard address
        // (lowering_rules.zig:4781/4793 inner-type probes).
        let guard_inner = user_defined_generic_inner(&source_ty, "MutexGuard")
            .or_else(|| user_defined_generic_inner(&source_ty, "RwLockWriteGuard"));
        if let Some(inner) = guard_inner {
            let target = self.gen_expr(source_expr)?;
            let value = self.gen_expr(&assign.value)?;
            crate::release_prim::emit_store(
                &mut self.state,
                target,
                0,
                value,
                storage_prim_type(&inner)?,
            );
            if !self.is_local_reg(value) {
                self.emit_release(value)?;
            }
            return Ok(());
        }
        let inner_ty = match &source_ty {
            Type::Borrow(inner) | Type::Pointer(inner) => (**inner).clone(),
            _ => {
                return Err(unsupported(
                    "deref assign target of non-borrow/pointer type",
                    "G gen_assign",
                    "10350-10355",
                ))
            }
        };
        let target = self.gen_expr(source_expr)?;
        let value = self.gen_expr(&assign.value)?;
        crate::release_prim::emit_store(
            &mut self.state,
            target,
            0,
            value,
            storage_prim_type(&inner_ty)?,
        );
        let target_lifecycle = plan_deref_assignment_target_lifecycle(
            !matches!(&**source_expr, Node::Identifier(_)),
            self.state.frame.refcell_borrow_values.contains_key(&target),
        );
        if target_lifecycle.should_release() {
            self.emit_release(target)?;
        }
        if !self.is_local_reg(value) {
            self.emit_release(value)?;
        }
        Ok(())
    }

    /// Zig `genAssign` field arm (10366–10382).
    fn gen_assign_field(&mut self, assign: &AssignStmt, field: &FieldExpr) -> Result<()> {
        let expr_ty = self.check.expr_type(&field.expr).cloned().ok_or_else(|| {
            Error::MissingType("field assign target has no checker type".to_string())
        })?;
        let field_ty = if let Type::Tuple(tuple) = &expr_ty {
            let index: usize = field.field_name.parse().map_err(|_| {
                unsupported(
                    "tuple field assign with non-numeric field name",
                    "G gen_assign",
                    "10369",
                )
            })?;
            tuple.elems.get(index).cloned().ok_or_else(|| {
                unsupported(
                    "tuple field assign index out of range",
                    "G gen_assign",
                    "10370",
                )
            })?
        } else {
            LayoutAdapter::new(self.check)
                .field_type(&expr_ty, &field.field_name)
                .ok_or_else(|| {
                    unsupported(
                        "field assign with unknown field type",
                        "G gen_assign",
                        "10372",
                    )
                })?
        };
        let target = self.gen_field_address(field)?;
        let value = self.gen_expr(&assign.value)?;
        crate::release_prim::emit_store(
            &mut self.state,
            target.reg,
            0,
            value,
            prim_type(&field_ty)?,
        );
        if !self.is_local_reg(value) {
            self.emit_release(value)?;
        }
        if !self.is_local_reg(target.reg) {
            self.emit_release(target.reg)?;
        }
        self.mark_assignment_moved_source_with_type(&assign.target, &assign.value, &field_ty)
    }

    /// Zig `genAssign` index arm (10384–10413).
    fn gen_assign_index(&mut self, assign: &AssignStmt, idx: &IndexExpr) -> Result<()> {
        let target_ty = self.check.expr_type(&idx.target).cloned().ok_or_else(|| {
            Error::MissingType("index assign target has no checker type".to_string())
        })?;
        if !matches!(target_ty, Type::Array(_)) {
            let target_type_name = type_base_name(&target_ty).ok_or_else(|| {
                unsupported(
                    "index assign target without a base type name",
                    "G gen_assign",
                    "10386",
                )
            })?;
            let rule = find_std_surface_rule(
                &self.state.program.std_surface_rules,
                StdSurfaceRuleKind::IndexAssign,
                &target_type_name,
                None,
            )
            .ok_or_else(|| {
                unsupported(
                    &format!("index assign surface rule for `{target_type_name}`"),
                    "G gen_assign",
                    "10387",
                )
            })?
            .clone();
            let target_reg = self.gen_expr(&idx.target)?;
            let index_reg = self.gen_expr(&idx.index)?;
            let value = self.gen_expr(&assign.value)?;
            let values = StdSurfaceValues {
                receiver: Some(target_reg),
                index: Some(index_reg),
                value: Some(value),
                elem_size: element_slot_size(&target_ty),
                elem_ty: Some(element_load_type(&target_ty)?),
                ..StdSurfaceValues::default()
            };
            self.emit_assign_rule(&rule, &values)?;
            self.release_non_local(target_reg)?;
            self.release_non_local(index_reg)?;
            self.release_non_local(value)?;
            return self.mark_assignment_moved_source(&assign.target, &assign.value);
        }
        let Type::Array(arr) = &target_ty else {
            unreachable!("array guard checked above")
        };
        let arr = arr.clone();
        let target = self.gen_index_address(idx, &arr)?;
        let value = self.gen_expr(&assign.value)?;
        crate::release_prim::emit_store(
            &mut self.state,
            target.reg,
            0,
            value,
            storage_prim_type(&arr.elem)?,
        );
        if !self.is_local_reg(value) {
            self.emit_release(value)?;
        }
        self.release_address_source(&target)?;
        self.mark_assignment_moved_source(&assign.target, &assign.value)
    }

    /// Zig `genFieldAddress` (11775–11786).
    fn gen_field_address(&mut self, field: &FieldExpr) -> Result<AddressSource> {
        let expr_ty = self.check.expr_type(&field.expr).cloned().ok_or_else(|| {
            Error::MissingType("field address target has no checker type".to_string())
        })?;
        let layout = if let Type::Tuple(tuple) = &expr_ty {
            let index: usize = field.field_name.parse().map_err(|_| {
                unsupported(
                    "tuple field address with non-numeric field name",
                    "G gen_assign",
                    "11778",
                )
            })?;
            tuple_field_layout(tuple, index).ok_or_else(|| {
                unsupported(
                    "tuple field address index out of range",
                    "G gen_assign",
                    "11779",
                )
            })?
        } else {
            LayoutAdapter::new(self.check).field_layout(&expr_ty, &field.field_name)?
        };
        let base = self.gen_expr(&field.expr)?;
        let mut source = self.address_with_offset(base, layout.offset);
        if layout.offset != 0 && !self.is_local_reg(base) {
            source.release_regs = vec![base];
        }
        Ok(source)
    }

    /// Zig `genIndexAddress` (11806–11903), array path only —
    /// see the module note for why the Slice / surface paths
    /// are unreachable from `genAssign`.
    fn gen_index_address(&mut self, idx: &IndexExpr, arr: &ArrayType) -> Result<AddressSource> {
        let mut target_source = if let Node::FieldExpr(f) = &*idx.target {
            self.gen_field_address(f)?
        } else {
            AddressSource {
                reg: self.gen_expr(&idx.target)?,
                release_regs: Vec::new(),
            }
        };
        let target_reg = target_source.reg;
        if let Node::Literal(Literal::IntVal(raw_index)) = &*idx.index {
            if *raw_index < 0 {
                return Err(unsupported(
                    "negative array index in assign target",
                    "G gen_assign",
                    "11883",
                ));
            }
            let layout = array_element_layout(arr, *raw_index as usize).ok_or_else(|| {
                unsupported(
                    "array index assign without an element layout",
                    "G gen_assign",
                    "11885",
                )
            })?;
            let mut source = self.address_with_offset(target_reg, layout.offset);
            if source.reg == target_reg {
                source.release_regs = std::mem::take(&mut target_source.release_regs);
            } else {
                let mut regs = Vec::new();
                if !self.is_local_reg(target_reg) {
                    regs.push(target_reg);
                }
                regs.append(&mut target_source.release_regs);
                source.release_regs = regs;
            }
            return Ok(source);
        }
        let index_reg = self.gen_expr(&idx.index)?;
        let (elem_ptr, offset) = self.gen_array_element_ptr(arr, target_reg, index_reg);
        if let Some(offset) = offset {
            self.emit_release(offset)?;
        }
        if !self.is_local_reg(index_reg) {
            self.emit_release(index_reg)?;
        }
        let mut regs = Vec::new();
        if !self.is_local_reg(target_reg) {
            regs.push(target_reg);
        }
        regs.append(&mut target_source.release_regs);
        Ok(AddressSource {
            reg: elem_ptr,
            release_regs: regs,
        })
    }

    /// Zig `genArrayElementPtr` (4016–4029).
    fn gen_array_element_ptr(
        &mut self,
        arr: &ArrayType,
        target_reg: u32,
        index_reg: u32,
    ) -> (u32, Option<u32>) {
        let stride = array_stride(&arr.elem);
        let elem_ptr = self.b().new_tmp();
        if stride == 1 {
            crate::release_prim::emit_ptr_add(
                &mut self.state,
                elem_ptr,
                target_reg,
                Operand::Reg(index_reg),
            );
            return (elem_ptr, None);
        }
        let offset = self.b().new_tmp();
        crate::release_prim::emit_op(
            &mut self.state,
            offset,
            OpKind::Mul,
            Operand::Reg(index_reg),
            Operand::ImmI64(stride as i64),
        );
        crate::release_prim::emit_ptr_add(
            &mut self.state,
            elem_ptr,
            target_reg,
            Operand::Reg(offset),
        );
        (elem_ptr, Some(offset))
    }

    /// Zig `addressWithOffset` (11768–11773).
    fn address_with_offset(&mut self, base: u32, offset: usize) -> AddressSource {
        if offset == 0 {
            return AddressSource {
                reg: base,
                release_regs: Vec::new(),
            };
        }
        let dst = self.b().new_tmp();
        crate::release_prim::emit_ptr_add(
            &mut self.state,
            dst,
            base,
            Operand::ImmU64(offset as u64),
        );
        AddressSource {
            reg: dst,
            release_regs: Vec::new(),
        }
    }

    /// Zig `releaseAddressSource` (11799–11804).
    fn release_address_source(&mut self, source: &AddressSource) -> Result<()> {
        if !self.is_local_reg(source.reg) {
            self.emit_release(source.reg)?;
        }
        for reg in &source.release_regs {
            self.emit_release(*reg)?;
        }
        Ok(())
    }

    /// Zig `markAssignmentMovedSource` (8853–8858). The Zig
    /// fallback chain is `localType` then `exprTypeOrFallback`;
    /// the latter's literal arm is unreachable here (the value
    /// in this path is an identifier), so the checker type map
    /// stands in for it.
    fn mark_assignment_moved_source(&mut self, target: &Node, value: &Node) -> Result<()> {
        let Node::Identifier(source_name) = value else {
            return Ok(());
        };
        let value_ty = crate::captures::local_type(&self.state, source_name)
            .or_else(|| self.check.expr_type(value).cloned())
            .ok_or_else(|| {
                Error::MissingType(format!(
                    "assignment moved source `{source_name}` has no type"
                ))
            })?;
        self.mark_assignment_moved_source_with_type(target, value, &value_ty)
    }

    /// Zig `markAssignmentMovedSourceWithType` (8860–8867).
    fn mark_assignment_moved_source_with_type(
        &mut self,
        target: &Node,
        value: &Node,
        stored_ty: &Type,
    ) -> Result<()> {
        if !matches!(value, Node::Identifier(_)) {
            return Ok(());
        }
        let mut facts = CopyFacts::new(self.check);
        let is_copy = facts.type_is_copy_value(stored_ty);
        let Some(moved_name) =
            sla_lowering::release_facts::assignment_moves_identifier(target, value, stored_ty, is_copy)
        else {
            return Ok(());
        };
        if let Some(source_reg) = self.state.frame.local_reg(moved_name) {
            crate::release_emit::emit_assignment_move(&mut self.state, source_reg);
        }
        Ok(())
    }

    /// Zig `emitStdSurfaceRule` (D4) with the production
    /// emitter; same queue/symbol plumbing as
    /// gen_surface_call's `emit_surface_rule` (per-module copy,
    /// as in gen_method_call.rs).
    fn emit_assign_rule(&mut self, rule: &StdSurfaceRule, values: &StdSurfaceValues) -> Result<()> {
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
