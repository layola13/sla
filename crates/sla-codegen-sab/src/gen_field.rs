//! G — `genField` / `genDeref`: the FieldExpr read arm and
//! its DerefExpr base companion. Zig `genField`
//! (sab_codegen.zig:24285) and `genDeref` (14890), ported
//! arm-by-arm; the two `baseIs*` through-load guards
//! (14151 / 14186) ride along because they are part of the
//! field read's base semantics.
//!
//! The value model the arms rely on (Zig `abiTypeSize`):
//! every non-primitive value's register denotes its storage
//! — an aggregate field occupies one pointer-sized slot
//! holding the field value's own storage address, so a
//! field read is a single typed load at the field offset.
//!
//! Deliberate subset stops (loud, never silent):
//! - smart-pointer deref (`genSmartPointerGet`): a deref
//!   whose source or result type is a Box/Rc/Arc value
//!   stops with that attribution; raw-pointer and borrow
//!   derefs (the shapes the FieldExpr corpus exercises)
//!   are ported in full.

use sla_ast::{BorrowExpr, DerefExpr, FieldExpr, Node, Type};
use sla_lowering::{abi_passes_as_pointer, is_borrow_like_type};

use crate::copy_facts::CopyFacts;
use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::ir::{CapPrefix, PrimType};
use crate::layout::{storage_prim_type, tuple_field_layout, user_defined_generic_inner, LayoutAdapter};
use crate::locals::{mark_object_ptr_reg, reg_is_object_ptr};
use crate::release_emit::mark_loaded_field_view_if_needed;
use crate::release_meta::{owned_release_regs, remember_borrow_address_temps};
use crate::types::AddressSource;

/// Zig `smartPointerDerefType` (lowering_rules.zig:5097)
/// reduced to a presence test: a user-defined generic named
/// Box / Rc / Arc (RefCell is excluded, as in Zig's switch;
/// the borrow/pointer peel matches `userDefinedGenericInner`).
fn smart_pointer_deref_present(ty: &Type) -> bool {
    user_defined_generic_inner(ty, "Box").is_some()
        || user_defined_generic_inner(ty, "Rc").is_some()
        || user_defined_generic_inner(ty, "Arc").is_some()
}

/// Zig `smartPointerType` (lowering_rules.zig:5089) reduced
/// to a presence test: Box / Rc / Arc / RefCell.
fn smart_pointer_present(ty: &Type) -> bool {
    smart_pointer_deref_present(ty) || user_defined_generic_inner(ty, "RefCell").is_some()
}

impl<'c> Driver<'c> {
    /// Zig `genField` (sab_codegen.zig:24285).
    pub(crate) fn gen_field(&mut self, field: &FieldExpr) -> Result<u32> {
        let expr_ty = self.check.expr_type(&field.expr).cloned().ok_or_else(|| {
            Error::MissingType("field base has no checker type".to_string())
        })?;
        // Tuple arm: numeric field name, layout walk, plain load.
        if let Type::Tuple(tuple) = &expr_ty {
            let index: usize = field.field_name.parse().map_err(|_| {
                unsupported(
                    "tuple field read with non-numeric field name",
                    "G gen_field",
                    "24285",
                )
            })?;
            let layout = tuple_field_layout(tuple, index).ok_or_else(|| {
                unsupported("tuple field read index out of range", "G gen_field", "24285")
            })?;
            let elem_ty = tuple.elems.get(index).cloned().ok_or_else(|| {
                unsupported("tuple field read index out of range", "G gen_field", "24285")
            })?;
            let base = self.gen_expr(&field.expr)?;
            let dst = self.b().new_tmp();
            self.emit_load(dst, base, layout.offset, layout.ty);
            mark_loaded_field_view_if_needed(&mut self.state, self.check, dst, &elem_ty);
            // DEF-DEREF-BORROW-WRITE: a borrow-typed element's
            // loaded word IS the pointee object pointer.
            if matches!(elem_ty, Type::Borrow(_)) {
                mark_object_ptr_reg(&mut self.state, dst);
            }
            if !self.is_local_reg(base) {
                self.emit_release(base)?;
            }
            return Ok(dst);
        }
        let layout = LayoutAdapter::new(self.check).field_layout(&expr_ty, &field.field_name)?;
        let field_ty = LayoutAdapter::new(self.check)
            .field_type(&expr_ty, &field.field_name)
            .ok_or_else(|| {
                unsupported("field read with unknown field type", "G gen_field", "24285")
            })?;

        let base = self.gen_expr(&field.expr)?;
        // DEF-CHAIN / CLASSB-DEF2 through-load: a raw-pointer
        // or pointer-passing borrow base's register holds the
        // address of the cell storing the object pointer, so
        // load the object pointer before any field offset
        // applies. Bases proven to hold the object pointer
        // directly skip it (object-ptr regs, borrow-param
        // object pointers, RefCell borrow handles, chan
        // slot-view borrows).
        let mut eff_base = base;
        let mut through: Option<u32> = None;
        let through_candidate = match &expr_ty {
            Type::Pointer(_) => true,
            Type::Borrow(inner) => {
                abi_passes_as_pointer(inner)
                    && !self.inline_pod_element(inner)
                    && !self.state.frame.refcell_borrow_values.contains_key(&base)
            }
            _ => false,
        };
        if through_candidate
            && !reg_is_object_ptr(&self.state, base)
            && !self.base_is_borrow_param_object_ptr(&field.expr, base)
            && !self.base_is_slot_view_borrow(field)
        {
            let obj = self.b().new_tmp();
            self.emit_load(obj, base, 0, PrimType::Ptr);
            mark_object_ptr_reg(&mut self.state, obj);
            eff_base = obj;
            through = Some(obj);
        }
        // An inline array field read as a value decays to its
        // address, exactly like the ManuallyDrop case below.
        if matches!(field_ty, Type::Array(_)) {
            let addr = self.b().new_tmp();
            self.emit_ptr_add_imm(addr, eff_base, layout.offset);
            mark_loaded_field_view_if_needed(&mut self.state, self.check, addr, &field_ty);
            if !self.is_local_reg(base) {
                self.emit_release(base)?;
            }
            if let Some(obj) = through {
                self.emit_release(obj)?;
            }
            return Ok(addr);
        }
        // Mirror SA-text: a ManuallyDrop-typed field decays to
        // its address (callers like `into_inner` load through
        // it); loading it as a value would misread the inline
        // payload as a pointer.
        if user_defined_generic_inner(&field_ty, "ManuallyDrop").is_some() {
            let addr = self.b().new_tmp();
            self.emit_ptr_add_imm(addr, eff_base, layout.offset);
            if !self.is_local_reg(base) {
                self.emit_release(base)?;
            }
            if let Some(obj) = through {
                self.emit_release(obj)?;
            }
            return Ok(addr);
        }
        let dst = self.b().new_tmp();
        self.emit_load(dst, eff_base, layout.offset, layout.ty);
        mark_loaded_field_view_if_needed(&mut self.state, self.check, dst, &field_ty);
        // DEF-DEREF-BORROW-WRITE: a borrow-typed field's loaded
        // word IS the pointee object pointer.
        if matches!(field_ty, Type::Borrow(_)) {
            mark_object_ptr_reg(&mut self.state, dst);
        }
        if !self.is_local_reg(base) {
            self.emit_release(base)?;
        }
        if let Some(obj) = through {
            self.emit_release(obj)?;
        }
        Ok(dst)
    }

    /// Zig `genDeref` (sab_codegen.zig:14890), minus the
    /// smart-pointer arm (see the module note).
    pub(crate) fn gen_deref(&mut self, expr: &Node, deref: &DerefExpr) -> Result<u32> {
        let deref_ty = self.check.expr_type(expr).cloned().ok_or_else(|| {
            Error::MissingType("deref expression has no checker type".to_string())
        })?;
        let source_ty = self.check.expr_type(&deref.expr).cloned().ok_or_else(|| {
            Error::MissingType("deref source has no checker type".to_string())
        })?;
        if smart_pointer_deref_present(&source_ty) || smart_pointer_deref_present(&deref_ty) {
            return Err(unsupported(
                "smart-pointer deref (genSmartPointerGet)",
                "G gen_deref",
                "14890",
            ));
        }
        let source = self.gen_expr(&deref.expr)?;
        // DEF-DEREF-BORROW-WRITE: dereferencing a borrow/pointer
        // to an aggregate yields the pointee OBJECT ADDRESS,
        // not a value load. A source proven to hold the object
        // pointer itself IS the deref result unchanged; a
        // cell-handle source loads through, and the loaded
        // pointer is registered so downstream place paths keep
        // the provenance. Scalar pointees keep the plain load.
        let aggregate_result = matches!(
            deref_ty,
            Type::UserDefined(_) | Type::Tuple(_) | Type::Array(_)
        );
        if aggregate_result && matches!(source_ty, Type::Borrow(_) | Type::Pointer(_)) {
            if reg_is_object_ptr(&self.state, source) {
                return Ok(source);
            }
            let loaded = self.b().new_tmp();
            self.emit_load(loaded, source, 0, PrimType::Ptr);
            self.mark_non_owning(loaded);
            mark_object_ptr_reg(&mut self.state, loaded);
            if !self.is_local_reg(source) {
                self.emit_release(source)?;
            }
            return Ok(loaded);
        }
        let dst = self.b().new_tmp();
        self.emit_load(dst, source, 0, storage_prim_type(&deref_ty)?);
        if !self.is_local_reg(source) {
            self.emit_release(source)?;
        }
        Ok(dst)
    }

    /// Zig `baseIsBorrowParamObjectPtr` (sab_codegen.zig:
    /// 14151): the base is an identifier whose most recent
    /// binding is a borrow-capability parameter holding an
    /// aggregate directly (its declared type is not itself a
    /// borrow).
    fn base_is_borrow_param_object_ptr(&self, expr: &Node, reg: u32) -> bool {
        let Node::Identifier(name) = expr else {
            return false;
        };
        for local in self.state.frame.locals.iter().rev() {
            if local.name != *name {
                continue;
            }
            return local.is_param
                && local.reg == reg
                && local.param_cap == Some(CapPrefix::Borrow)
                && !matches!(local.ty, Some(Type::Borrow(_)));
        }
        false
    }

    /// Zig `baseIsSlotViewBorrow` (sab_codegen.zig:14186):
    /// the chan slot-view borrow (`&slot as &__go_slot_view_N`,
    /// synthesized by the syntax lowering) holds the staging
    /// slot's address itself, never a cell handle.
    fn base_is_slot_view_borrow(&self, field: &FieldExpr) -> bool {
        if !matches!(&*field.expr, Node::Identifier(_)) {
            return false;
        }
        let Some(base_ty) = self.check.expr_type(&field.expr) else {
            return false;
        };
        let Type::Borrow(pointee) = base_ty else {
            return false;
        };
        let Type::UserDefined(ud) = &**pointee else {
            return false;
        };
        ud.name.starts_with("__go_slot_view_")
    }
}

impl<'c> Driver<'c> {
    /// Zig `genPrefixedBorrowAddressCallArg` (sab_codegen.zig:
    /// 19961), for the plain static-call tail: a borrow arg
    /// renders as `&place` in the call body — the host
    /// verifier reads the cap off the glyph — so no borrow
    /// register is materialized for the argument. Post-call
    /// releases are collected into `post` (Zig's
    /// SabLoweredCallArg release_regs, filtered through
    /// ownedReleaseRegs).
    pub(crate) fn gen_prefixed_borrow_call_arg(
        &mut self,
        borrow: &BorrowExpr,
        post: &mut Vec<u32>,
    ) -> Result<String> {
        let inner: &Node = &borrow.expr;
        if let Node::FieldExpr(field) = inner {
            let inner_ty = self.check.expr_type(inner).cloned();
            if let Some(ty) = &inner_ty {
                if self.field_borrow_loads_stored_pointer(ty)
                    || self.field_borrow_is_box_cell(ty)
                {
                    let projection = self.gen_field_address(field)?;
                    let owner = self.b().new_tmp();
                    self.b().record_reg(owner);
                    self.emit_load(owner, projection.reg, 0, PrimType::Ptr);
                    self.mark_non_owning(owner);
                    post.push(owner);
                    post.extend(self.field_projection_release_regs(&projection));
                    return Ok(format!("&{}", self.b().symbol_name(owner)));
                }
            }
        }
        if let Node::IndexExpr(index) = inner {
            let inner_ty = self.check.expr_type(inner).cloned();
            if let Some(ty) = &inner_ty {
                if self.index_borrow_loads_stored_pointer(ty) {
                    let source = self.gen_index_address(index)?;
                    let owner = self.b().new_tmp();
                    self.b().record_reg(owner);
                    self.emit_load(owner, source.reg, 0, PrimType::Ptr);
                    self.mark_non_owning(owner);
                    post.push(owner);
                    post.extend(self.field_projection_release_regs(&source));
                    return Ok(format!("&{}", self.b().symbol_name(owner)));
                }
            }
        }
        let mut source = self.gen_address_of(inner)?;
        if matches!(inner, Node::FieldExpr(_) | Node::IndexExpr(_)) {
            source = self.borrow_place_source(source)?;
        }
        // borrowAddressCallArgReleaseRegs('&'): release the
        // address value after the call when it is a temp,
        // plus the source temps, filtered to owned regs. The
        // ported gen_address_of never produces a restore
        // slot, so the plan's restore arm is inert here.
        let mut regs: Vec<u32> = Vec::new();
        if !self.is_local_reg(source.reg) {
            regs.push(source.reg);
        }
        regs.extend(source.release_regs.iter().copied());
        for reg in owned_release_regs(&regs) {
            post.push(reg);
        }
        Ok(format!("&{}", self.b().symbol_name(source.reg)))
    }

    /// Zig `fieldBorrowLoadsStoredPointer`
    /// (sab_codegen.zig:19826): a borrow of a pointer-backed
    /// owning field denotes the stored buffer pointer, not
    /// the address of the field slot.
    fn field_borrow_loads_stored_pointer(&mut self, field_ty: &Type) -> bool {
        if !crate::aggregate::struct_field_is_pointer_backed(field_ty) {
            return false;
        }
        if smart_pointer_present(field_ty) {
            return false;
        }
        let mut facts = CopyFacts::new(self.check);
        let loads = (!facts.type_is_copy_value(field_ty) || facts.type_is_copy_struct(field_ty))
            && !is_borrow_like_type(field_ty);
        loads
    }

    /// Zig `fieldBorrowIsBoxCell` (sab_codegen.zig:19851).
    fn field_borrow_is_box_cell(&self, field_ty: &Type) -> bool {
        user_defined_generic_inner(field_ty, "Box").is_some()
    }

    /// Zig `indexBorrowLoadsStoredPointer`
    /// (sab_codegen.zig:19857).
    fn index_borrow_loads_stored_pointer(&mut self, elem_ty: &Type) -> bool {
        if self.inline_pod_element(elem_ty) {
            return false;
        }
        self.field_borrow_loads_stored_pointer(elem_ty)
    }

    /// Zig `fieldProjectionReleaseRegs`
    /// (sab_codegen.zig:19866) over the port's
    /// `gen_assign::AddressSource`.
    fn field_projection_release_regs(&self, projection: &crate::gen_assign::AddressSource) -> Vec<u32> {
        let mut regs = Vec::new();
        if !self.is_local_reg(projection.reg) {
            regs.push(projection.reg);
        }
        for reg in &projection.release_regs {
            if *reg != projection.reg {
                regs.push(*reg);
            }
        }
        owned_release_regs(&regs)
    }

    /// Zig `genAddressOf` (sab_codegen.zig:14635), subset:
    /// the place shapes the borrow tail reaches (identifier /
    /// field / index / deref / value temp). Smart-pointer
    /// deref places stop loudly, as in `gen_deref`.
    pub(crate) fn gen_address_of(&mut self, expr: &Node) -> Result<AddressSource> {
        match expr {
            Node::Identifier(name) => {
                if let Some(slot) = crate::captures::stack_local(&self.state, name) {
                    return Ok(AddressSource {
                        reg: slot.reg,
                        release_regs: Vec::new(),
                        restore_slot: None,
                    });
                }
                let reg = self.gen_expr(expr)?;
                Ok(AddressSource {
                    reg,
                    release_regs: Vec::new(),
                    restore_slot: None,
                })
            }
            Node::FieldExpr(field) => {
                let source = self.gen_field_address(field)?;
                Ok(AddressSource {
                    reg: source.reg,
                    release_regs: source.release_regs,
                    restore_slot: None,
                })
            }
            Node::IndexExpr(idx) => {
                let source = self.gen_index_address(idx)?;
                Ok(AddressSource {
                    reg: source.reg,
                    release_regs: source.release_regs,
                    restore_slot: None,
                })
            }
            Node::DerefExpr(deref) => {
                let deref_ty = self.check.expr_type(expr).cloned().ok_or_else(|| {
                    Error::MissingType("deref place has no checker type".to_string())
                })?;
                let source_ty = self.check.expr_type(&deref.expr).cloned().ok_or_else(|| {
                    Error::MissingType("deref place source has no checker type".to_string())
                })?;
                if smart_pointer_present(&source_ty) || smart_pointer_present(&deref_ty) {
                    return Err(unsupported(
                        "borrow of a smart-pointer deref place",
                        "G gen_borrow",
                        "14635",
                    ));
                }
                let aggregate = matches!(
                    deref_ty,
                    Type::UserDefined(_) | Type::Tuple(_) | Type::Array(_)
                );
                if aggregate && matches!(source_ty, Type::Borrow(_) | Type::Pointer(_)) {
                    // DEF-5/D-1: the place `*p` lives at the
                    // address stored in p's cell — the same
                    // load genDeref uses as the field base.
                    let source = self.gen_expr(&deref.expr)?;
                    if reg_is_object_ptr(&self.state, source) {
                        return Ok(AddressSource {
                            reg: source,
                            release_regs: Vec::new(),
                            restore_slot: None,
                        });
                    }
                    let loaded = self.b().new_tmp();
                    self.emit_load(loaded, source, 0, PrimType::Ptr);
                    self.mark_non_owning(loaded);
                    mark_object_ptr_reg(&mut self.state, loaded);
                    return Ok(AddressSource {
                        reg: loaded,
                        release_regs: if self.is_local_reg(source) {
                            Vec::new()
                        } else {
                            vec![source]
                        },
                        restore_slot: None,
                    });
                }
                let reg = self.gen_expr(&deref.expr)?;
                Ok(AddressSource {
                    reg,
                    release_regs: Vec::new(),
                    restore_slot: None,
                })
            }
            _ => {
                let reg = self.gen_expr(expr)?;
                Ok(AddressSource {
                    reg,
                    release_regs: Vec::new(),
                    restore_slot: None,
                })
            }
        }
    }

    /// Zig `borrowPlaceSource` (sab_codegen.zig:14747): a
    /// field/index place borrow targets a fresh ptr-add-0
    /// alias of the place address (the port's sources never
    /// carry a restore slot, so Zig's early return is kept
    /// for parity only).
    fn borrow_place_source(&mut self, source: AddressSource) -> Result<AddressSource> {
        if source.restore_slot.is_some() {
            return Ok(source);
        }
        let place = self.b().new_tmp();
        self.emit_ptr_add_imm(place, source.reg, 0);
        if reg_is_object_ptr(&self.state, source.reg) {
            mark_object_ptr_reg(&mut self.state, place);
        }
        let mut regs = Vec::new();
        if !self.is_local_reg(source.reg) {
            regs.push(source.reg);
        }
        for reg in &source.release_regs {
            if *reg != source.reg {
                regs.push(*reg);
            }
        }
        Ok(AddressSource {
            reg: place,
            release_regs: owned_release_regs(&regs),
            restore_slot: None,
        })
    }

    /// Zig `genBorrow` (sab_codegen.zig:14762). The
    /// `cell_target_borrows` registration has no counterpart
    /// in this port's frame (the set itself was not ported),
    /// so the DEF-SP2 deref-receiver hop it feeds stays with
    /// the smart-pointer family and out of this subset.
    pub(crate) fn gen_borrow(&mut self, borrow: &BorrowExpr) -> Result<u32> {
        // Borrow of a pointer-backed owning field: load the
        // stored pointer first and borrow that (Y-SHARE parity
        // with SA-text `&field`).
        if let Node::FieldExpr(field) = &*borrow.expr {
            if let Some(inner_ty) = self.check.expr_type(&borrow.expr).cloned() {
                if self.field_borrow_is_box_cell(&inner_ty) {
                    return Err(unsupported(
                        "borrow of a Box field place (cell denotation)",
                        "G gen_borrow",
                        "14762",
                    ));
                }
                if self.field_borrow_loads_stored_pointer(&inner_ty) {
                    let projection = self.gen_field_address(field)?;
                    let release_regs = self.field_projection_release_regs(&projection);
                    let owner = self.b().new_tmp();
                    self.emit_load(owner, projection.reg, 0, PrimType::Ptr);
                    self.mark_non_owning(owner);
                    mark_object_ptr_reg(&mut self.state, owner);
                    let dst = self.b().new_tmp();
                    self.emit_borrow_reg(dst, owner, "read");
                    mark_object_ptr_reg(&mut self.state, dst);
                    remember_borrow_address_temps(
                        &mut self.state,
                        dst,
                        &AddressSource {
                            reg: owner,
                            release_regs,
                            restore_slot: None,
                        },
                    );
                    return Ok(dst);
                }
            }
        }
        // DEF-SAB-BORROW-RESIDUAL: index twin of the field
        // branch (inline-POD elements are exempt — their
        // record is packed in the slot itself).
        if let Node::IndexExpr(idx) = &*borrow.expr {
            if let Some(elem_ty) = self.check.expr_type(&borrow.expr).cloned() {
                if self.field_borrow_is_box_cell(&elem_ty) {
                    return Err(unsupported(
                        "borrow of a Box element place (cell denotation)",
                        "G gen_borrow",
                        "14762",
                    ));
                }
                if self.index_borrow_loads_stored_pointer(&elem_ty) {
                    let place = self.gen_index_address(idx)?;
                    let release_regs = self.field_projection_release_regs(&place);
                    let owner = self.b().new_tmp();
                    self.emit_load(owner, place.reg, 0, PrimType::Ptr);
                    self.mark_non_owning(owner);
                    mark_object_ptr_reg(&mut self.state, owner);
                    let dst = self.b().new_tmp();
                    self.emit_borrow_reg(dst, owner, "read");
                    mark_object_ptr_reg(&mut self.state, dst);
                    remember_borrow_address_temps(
                        &mut self.state,
                        dst,
                        &AddressSource {
                            reg: owner,
                            release_regs,
                            restore_slot: None,
                        },
                    );
                    return Ok(dst);
                }
            }
        }
        let mut source = self.gen_address_of(&borrow.expr)?;
        if matches!(&*borrow.expr, Node::FieldExpr(_) | Node::IndexExpr(_)) {
            source = self.borrow_place_source(source)?;
        }
        let dst = self.b().new_tmp();
        self.emit_borrow_reg(dst, source.reg, "read");
        // DEF-SAB-BORROW-WRITE-NULL: provenance propagates
        // from the source register.
        if reg_is_object_ptr(&self.state, source.reg) {
            mark_object_ptr_reg(&mut self.state, dst);
        }
        remember_borrow_address_temps(&mut self.state, dst, &source);
        Ok(dst)
    }
}
