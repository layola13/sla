//! Place-address emission, mirrored from codegen.zig's
//! `genIndexAddress` (16691), `genFieldAddress` (16827),
//! `fieldAddressLayout`, `finishIndexAddress`,
//! `releaseIndexSlotBase`, and the lowering_rules plans they
//! consult (`fieldAddressProjectionTracksSourceTemp`,
//! `planBorrowAddressTemps`, `structDeclIsInlinePod`).
//!
//! An address computation leaves the derived pointer in `ptr`;
//! the bookkeeping fields record which temps the consumer must
//! retire, and in which order, once it is done with the address
//! (Zig's IndexAddress / AddressProjection structs). The borrow
//! consumer in into.rs additionally gates on `source_temp`: the
//! slice maintains no `borrow_source_temps` table, so a field
//! projection whose base is a temp stays loud there rather than
//! silently leaking the temp (see gen_borrow_expr).

use sla_ast::{FieldExpr, IndexExpr, Node, StructDecl, Type};
use sla_lowering::{call_arg_needs_release, is_temporary_register_name, peel_borrow_pointer_type};

use crate::codegen::{type_size, vec_element_slot_size, Codegen};
use crate::layout;
use crate::{unsupported, Result, TextError};

/// codegen.zig `IndexAddress`.
#[derive(Debug, Clone)]
pub(crate) struct IndexAddress {
    pub ptr: String,
    pub elem_ty: Type,
    pub base_tmp: Option<String>,
    pub base_reg: String,
    pub release_base_reg: bool,
    pub slot_base_reg: Option<String>,
}

/// codegen.zig `AddressProjection`.
#[derive(Debug, Clone)]
pub(crate) struct FieldAddress {
    pub ptr: String,
    pub source_temp: Option<String>,
}

/// codegen.zig `VecReceiver`: the settled owner register for a
/// Vec/GrowVec index operation — the register to project through,
/// the register to release after the operation (if any), and the
/// register to forget after the operation (if any).
#[derive(Debug, Clone)]
pub(crate) struct VecReceiver {
    pub reg: String,
    pub release_reg: Option<String>,
    pub consume_reg: Option<String>,
}

/// Zig `vecLikeElementType` + `growVecElementType` discrimination:
/// (is_grow, element type) for a Vec/GrowVec target type.
pub(crate) fn vec_like_element_type(ty: &Type) -> Option<(bool, Type)> {
    match peel_borrow_pointer_type(ty) {
        Type::UserDefined(u) if u.name == "Vec" || u.name == "GrowVec" => {
            u.generics.first().cloned().map(|e| (u.name == "GrowVec", e))
        }
        _ => None,
    }
}

/// Zig `sliceElementType`.
pub(crate) fn slice_element_type(ty: &Type) -> Option<Type> {
    match peel_borrow_pointer_type(ty) {
        Type::UserDefined(u) if u.name == "Slice" => u.generics.first().cloned(),
        _ => None,
    }
}

/// The struct-decl lookup of into.rs `struct_decl_for_name`
/// (module-private there): exact name, then the local name after
/// the last `.` / `::`.
pub(crate) fn local_name(name: &str) -> &str {
    name.rsplit(['.', ':']).next().unwrap_or(name)
}

pub(crate) fn struct_decl_lookup(check: &dyn sla_lowering::CheckInfo, name: &str) -> Option<StructDecl> {
    if let Some(decl) = check.struct_decl(name) {
        return Some(decl.clone());
    }
    let dot = name.rfind('.').map(|i| i + 1).unwrap_or(0);
    let colon = name.rfind("::").map(|i| i + 2).unwrap_or(0);
    let start = dot.max(colon);
    if start == 0 || start >= name.len() {
        return None;
    }
    check.struct_decl(&name[start..]).cloned()
}

/// Zig `inlinePodElement` over `structDeclIsInlinePod`: a
/// non-opaque, non-union, non-generic struct whose fields are
/// all primitives and whose packed size is exactly one word.
/// (Same fact the SAB crate computes in gen_method_call.rs.)
pub(crate) fn inline_pod_element(check: &dyn sla_lowering::CheckInfo, ty: &Type) -> bool {
    let Type::UserDefined(u) = ty else {
        return false;
    };
    let Some(decl) = struct_decl_lookup(check, &u.name) else {
        return false;
    };
    if decl.is_opaque || decl.is_union || !decl.generics.is_empty() || decl.fields.is_empty() {
        return false;
    }
    if decl
        .fields
        .iter()
        .any(|f| !matches!(&*f.ty, Type::Primitive(_)))
    {
        return false;
    }
    layout::struct_size(&decl) == 8
}

impl<'c> Codegen<'c> {
    /// Whether an index expression addresses an inline-POD
    /// element (Zig `indexExprIsInlinePodElement`).
    pub(crate) fn index_expr_is_inline_pod_element(&self, expr: &Node) -> bool {
        let Node::IndexExpr(idx) = expr else {
            return false;
        };
        let Some(target_ty) = self.expr_type(&idx.target) else {
            return false;
        };
        let elem = vec_like_element_type(&target_ty)
            .map(|(_, e)| e)
            .or_else(|| slice_element_type(&target_ty))
            .or_else(|| match peel_borrow_pointer_type(&target_ty) {
                Type::Array(arr) => Some((*arr.elem).clone()),
                _ => None,
            });
        match elem {
            Some(elem) => inline_pod_element(self.check, &elem),
            None => false,
        }
    }

    /// codegen.zig:16691 `genIndexAddress`.
    pub(crate) fn gen_index_address(&mut self, idx: &IndexExpr) -> Result<IndexAddress> {
        let target_ty = self
            .place_type_of_expr(&idx.target)
            .ok_or_else(|| TextError::MissingType {
                context: "index target type".to_string(),
            })?;

        if let Some((_, elem_ty)) = vec_like_element_type(&target_ty) {
            // CLASSB-DEF2: the element address is its slot in the
            // container buffer (handle+0, frozen layout; slot
            // stride = vecElementSlotSize). The buffer base is NOT
            // released here — it travels in slot_base_reg and is
            // released by finishIndexAddress / releaseIndexSlotBase
            // (DEF-SLAREDIS-BUILD-MAIN).
            let vec_reg = self.gen_expr(&idx.target)?;
            let base_reg = self.new_tmp();
            self.ins(&format!("{base_reg} = load {vec_reg}+0 as ptr"));
            let index_reg = self.gen_expr(&idx.index)?;
            let offset_reg = self.new_tmp();
            self.ins(&format!(
                "{offset_reg} = mul {index_reg}, {}",
                vec_element_slot_size(&elem_ty)
            ));
            let ptr_reg = self.new_tmp();
            self.ins(&format!("{ptr_reg} = ptr_add {base_reg}, {offset_reg}"));
            self.emit_release(&offset_reg)?;
            if self.expr_result_reg_needs_release(&idx.index) {
                self.emit_release(&index_reg)?;
            }
            let release_base_reg = sla_lowering::expr_result_needs_release(&idx.target)
                || is_temporary_register_name(&vec_reg);
            return Ok(IndexAddress {
                ptr: ptr_reg,
                elem_ty,
                base_tmp: None,
                base_reg: vec_reg,
                release_base_reg,
                slot_base_reg: Some(base_reg),
            });
        }

        if let Some(elem_ty) = slice_element_type(&target_ty) {
            let slice_reg = self.gen_expr(&idx.target)?;
            let base_reg = self.new_tmp();
            self.ins(&format!("{base_reg} = load {slice_reg}+Slice_ptr as ptr"));
            let index_reg = self.gen_expr(&idx.index)?;
            let offset_reg = self.new_tmp();
            self.ins(&format!(
                "{offset_reg} = mul {index_reg}, {}",
                type_size(&elem_ty)
            ));
            let ptr_reg = self.new_tmp();
            self.ins(&format!("{ptr_reg} = ptr_add {base_reg}, {offset_reg}"));
            self.emit_release(&offset_reg)?;
            if self.expr_result_reg_needs_release(&idx.index) {
                self.emit_release(&index_reg)?;
            }
            let release_base_reg = sla_lowering::expr_result_needs_release(&idx.target)
                || is_temporary_register_name(&slice_reg);
            return Ok(IndexAddress {
                ptr: ptr_reg,
                elem_ty,
                base_tmp: None,
                base_reg: slice_reg,
                release_base_reg,
                slot_base_reg: Some(base_reg),
            });
        }

        let arr = match peel_borrow_pointer_type(&target_ty) {
            Type::Array(arr) => arr.clone(),
            _ => {
                return unsupported(
                    "index address of non-vec/slice/array target",
                    "codegen.zig:16691 (genIndexAddress)",
                )
            }
        };
        // Array values in this crate are pointer-backed: a field
        // or binding of array type evaluates (gen_expr) to the
        // data pointer, which is exactly the indexing base.
        let base_source_reg = self.gen_expr(&idx.target)?;
        let base_tmp = match &*idx.target {
            Node::Identifier(name) if self.global_const_bindings.contains(name) => {
                let addr_reg = self.new_tmp();
                self.ins(&format!("{addr_reg} = &{name}"));
                Some(addr_reg)
            }
            _ => None,
        };
        let base_reg = base_tmp.clone().unwrap_or_else(|| base_source_reg.clone());
        let index_reg = self.gen_expr(&idx.index)?;
        let offset_reg = self.new_tmp();
        self.ins(&format!(
            "{offset_reg} = mul {index_reg}, {}",
            type_size(&arr.elem)
        ));
        let ptr_reg = self.new_tmp();
        self.ins(&format!("{ptr_reg} = ptr_add {base_reg}, {offset_reg}"));
        self.emit_release(&offset_reg)?;
        if self.expr_result_reg_needs_release(&idx.index) {
            self.emit_release(&index_reg)?;
        }
        let release_base_reg = base_tmp.is_none()
            && (sla_lowering::expr_result_needs_release(&idx.target)
                || is_temporary_register_name(&base_source_reg));
        Ok(IndexAddress {
            ptr: ptr_reg,
            elem_ty: (*arr.elem).clone(),
            base_tmp,
            base_reg: base_source_reg,
            release_base_reg,
            slot_base_reg: None,
        })
    }

    /// codegen.zig `finishIndexAddress`: release the address,
    /// then the deferred slot base, the global-const base temp,
    /// and the source register when the plan says so.
    pub(crate) fn finish_index_address(&mut self, addr: &IndexAddress) -> Result<()> {
        self.emit_release(&addr.ptr)?;
        if let Some(slot_base) = &addr.slot_base_reg {
            self.emit_release(slot_base)?;
        }
        if let Some(base_tmp) = &addr.base_tmp {
            self.emit_release(base_tmp)?;
        }
        if addr.release_base_reg {
            self.emit_release(&addr.base_reg)?;
        }
        Ok(())
    }

    /// codegen.zig `releaseIndexSlotBase`: call sites that use
    /// an IndexAddress without finishIndexAddress release the
    /// deferred slot base explicitly once done with the address.
    pub(crate) fn release_index_slot_base(&mut self, addr: &IndexAddress) -> Result<()> {
        if let Some(slot_base) = &addr.slot_base_reg {
            self.emit_release(slot_base)?;
        }
        Ok(())
    }

    /// codegen.zig `fieldAddressLayout`: tuple index or struct
    /// field layout over the peeled target type.
    pub(crate) fn field_address_layout(
        &self,
        target_ty: &Type,
        field_name: &str,
    ) -> Result<(usize, Type)> {
        let curr = peel_borrow_pointer_type(target_ty);
        match curr {
            Type::Tuple(tuple) => {
                let index: usize = field_name.parse().map_err(|_| TextError::Unsupported {
                    feature: format!("tuple field name `{field_name}`"),
                    zig_ref: "codegen.zig (fieldAddressLayout)",
                })?;
                layout::tuple_field_layout(&tuple.elems, index).ok_or_else(|| {
                    TextError::Unsupported {
                        feature: format!("tuple field layout {index}"),
                        zig_ref: "lowering_rules.zig (tupleFieldLayout)",
                    }
                })
            }
            Type::UserDefined(u) => {
                let decl = struct_decl_lookup(self.check, &u.name).ok_or_else(|| {
                    TextError::Unsupported {
                        feature: format!("field address: no struct decl for {}", u.name),
                        zig_ref: "codegen.zig (fieldAddressLayout)",
                    }
                })?;
                layout::struct_field_layout(&decl, field_name).ok_or_else(|| {
                    TextError::Unsupported {
                        feature: format!("struct field layout `{field_name}`"),
                        zig_ref: "lowering_rules.zig (structFieldLayout)",
                    }
                })
            }
            _ => unsupported(
                "field address on non-aggregate base",
                "codegen.zig (fieldAddressLayout)",
            ),
        }
    }

    /// codegen.zig `rememberAddressProjectionSource`: record the
    /// projection's source temp (planBorrowAddressTemps over
    /// `source_temp != null`) so emitRelease's borrow cascade can
    /// settle it when the address register is released.
    pub(crate) fn remember_address_projection_source(&mut self, projection: &FieldAddress) {
        if let Some(source_temp) = &projection.source_temp {
            self.borrow_source_temps
                .insert(projection.ptr.clone(), source_temp.clone());
        }
    }

    /// codegen.zig `rememberIndexAddressSource`.
    pub(crate) fn remember_index_address_source(&mut self, address: &IndexAddress) {
        let source_temp = address.base_tmp.clone().or_else(|| {
            if address.release_base_reg {
                Some(address.base_reg.clone())
            } else {
                None
            }
        });
        if let Some(source_temp) = source_temp {
            self.borrow_source_temps
                .insert(address.ptr.clone(), source_temp);
        }
    }

    /// codegen.zig `genVecOwnerReceiver`: the owner register for
    /// a Vec/GrowVec index operation. A field-projection target
    /// is read through its address — the owner word is loaded
    /// into a fresh temp and projected through a borrow of it, so
    /// the base binding's register is never opened as a
    /// composite. (A direct value load off the base register
    /// marks the base Composite; when the base is a borrowed
    /// parameter whose function-exit cleanup plan transfers to a
    /// result rooted at it, nothing retires the base and the
    /// host reports MemoryLeak at the return —
    /// RUST-TEXT-VECINDEX-RETURN-SELF-LEAK.) Any other target
    /// evaluates directly; its result register is the release
    /// candidate under the usual release fact.
    pub(crate) fn gen_vec_owner_receiver(&mut self, target: &Node) -> Result<VecReceiver> {
        if let Node::FieldExpr(field) = target {
            let projection = self.gen_field_address(field)?;
            self.remember_address_projection_source(&projection);
            let owner = self.new_tmp();
            self.ins(&format!("{owner} = load {}+0 as ptr", projection.ptr));
            self.emit_release(&projection.ptr)?;
            let borrowed = self.new_tmp();
            self.ins(&format!("{borrowed} = &{owner}"));
            return Ok(VecReceiver {
                reg: borrowed.clone(),
                release_reg: Some(borrowed),
                consume_reg: Some(owner),
            });
        }
        let reg = self.gen_expr(target)?;
        let release_reg = if self.expr_result_reg_needs_release(target) {
            Some(reg.clone())
        } else {
            None
        };
        Ok(VecReceiver {
            reg,
            release_reg,
            consume_reg: None,
        })
    }

    /// codegen.zig:16827 `genFieldAddress`.
    pub(crate) fn gen_field_address(&mut self, field: &FieldExpr) -> Result<FieldAddress> {
        let expr_ty = self.require_expr_type(&field.expr, "field base type")?;
        let base: String = if matches!(expr_ty, Type::Pointer(_)) {
            // DEF-CHAIN: the base value is a raw pointer — genExpr
            // yields its cell handle, so load the object pointer
            // before the field offset applies. The handle temp is
            // retired exactly like a deref source temp; the object
            // register takes the base's place in the projection
            // bookkeeping (a recorded borrow source moves with it).
            let handle = self.gen_expr(&field.expr)?;
            let obj = self.new_tmp();
            self.ins(&format!("{obj} = load {handle}+0 as ptr"));
            if let Some(source_temp) = self.borrow_source_temps.remove(&handle) {
                self.borrow_source_temps.insert(obj.clone(), source_temp);
            }
            if self.expr_result_reg_needs_release(&field.expr) {
                self.emit_release(&handle)?;
            }
            obj
        } else if matches!(&*field.expr, Node::FieldExpr(_)) {
            let nested_field = match &*field.expr {
                Node::FieldExpr(f) => f,
                _ => unreachable!(),
            };
            let nested = self.gen_field_address(nested_field)?;
            let nested_ty = self.require_expr_type(&field.expr, "nested field type")?;
            if matches!(nested_ty, Type::Pointer(_))
                || matches!(nested_ty, Type::Primitive(sla_ast::Primitive::RawPtr))
            {
                let loaded = self.new_tmp();
                self.ins(&format!("{loaded} = load {}+0 as ptr", nested.ptr));
                self.emit_release(&nested.ptr)?;
                if let Some(source_temp) = &nested.source_temp {
                    self.borrow_source_temps
                        .insert(loaded.clone(), source_temp.clone());
                }
                loaded
            } else {
                // Pointer-backed nested field: the field word in
                // the parent record is the nested record's own
                // pointer, so the outer projection's base is the
                // loaded word — exactly how gen_field_expr reads
                // the nested value. A ManuallyDrop field is the
                // exception: its value is the slot address itself.
                let nested_base_ty =
                    self.require_expr_type(&nested_field.expr, "nested field base type")?;
                let (_, nested_field_ty) =
                    self.field_address_layout(&nested_base_ty, &nested_field.field_name)?;
                if matches!(&nested_field_ty, Type::UserDefined(u) if local_name(&u.name) == "ManuallyDrop")
                {
                    self.remember_address_projection_source(&nested);
                    nested.ptr.clone()
                } else {
                    let loaded = self.new_tmp();
                    self.ins(&format!("{loaded} = load {}+0 as ptr", nested.ptr));
                    self.emit_release(&nested.ptr)?;
                    if let Some(source_temp) = &nested.source_temp {
                        self.borrow_source_temps
                            .insert(loaded.clone(), source_temp.clone());
                    }
                    loaded
                }
            }
        } else if matches!(&*field.expr, Node::IndexExpr(_))
            && self.index_expr_is_inline_pod_element(&field.expr)
        {
            // CLASSB segment 3: a field projection on an inline-POD
            // element. The base depends on the container's storage
            // model. ARRAY and SLICE slots genuinely hold the packed
            // record image, so the projection addresses the slot
            // itself. A Vec/GrowVec slot instead holds the element's
            // record pointer — VEC_PUSH stores the pointer the value
            // materialises as and the index READ path loads it back
            // (the DEF-RUST-TEXT-VEC-ELEMENT-BORROW-ARG settlement
            // for &v[i]: inline POD describes the element's packed
            // LAYOUT, not its Vec storage) — so a Vec/GrowVec
            // projection loads the stored pointer and addresses
            // through it. Addressing the Vec slot as a packed image
            // overwrote the pointer's high bytes and corrupted the
            // element (DEF-RUST-CLASSB-FIELDWRITE-STORAGE-MODEL).
            let idx = match &*field.expr {
                Node::IndexExpr(i) => i,
                _ => unreachable!(),
            };
            let addr = self.gen_index_address(idx)?;
            if let Some(t) = &addr.base_tmp {
                self.emit_release(t)?;
            }
            let target_ty = self.place_type_of_expr(&idx.target);
            let vec_like = target_ty
                .as_ref()
                .map(|t| vec_like_element_type(t).is_some())
                .unwrap_or(false);
            let base = if vec_like {
                let rec = self.new_tmp();
                self.ins(&format!("{rec} = load {}+0 as ptr", addr.ptr));
                self.emit_release(&addr.ptr)?;
                rec
            } else {
                addr.ptr.clone()
            };
            self.release_index_slot_base(&addr)?;
            if addr.release_base_reg {
                self.emit_release(&addr.base_reg)?;
            }
            base
        } else {
            self.gen_expr(&field.expr)?
        };
        let (offset, _) = self.field_address_layout(&expr_ty, &field.field_name)?;
        let ptr = self.new_tmp();
        self.ins(&format!("{ptr} = ptr_add {base}, {offset}"));
        let base_is_resolved_binding = match &*field.expr {
            Node::Identifier(name) => base == self.resolve_binding_name(name),
            _ => false,
        };
        // lowering_rules.fieldAddressProjectionTracksSourceTemp.
        let track = matches!(&*field.expr, Node::FieldExpr(_))
            || sla_lowering::expr_result_needs_release(&field.expr)
            || (is_temporary_register_name(&base) && !base_is_resolved_binding);
        Ok(FieldAddress {
            ptr,
            source_temp: if track { Some(base) } else { None },
        })
    }

    /// Assignment bookkeeping for the plain-rename branch of
    /// gen_assign_stmt: renaming a non-primitive identifier
    /// source is a MOVE at the host — the source register is
    /// Consumed by the assignment itself, so scope cleanup must
    /// not release it again (the loop-body cleanup for
    /// `boxed = next` emitted a second `!next`, UseAfterMove).
    /// Primitive sources go through emit_primitive_copy and
    /// stay live; materialised-copy sources arrive as temps and
    /// do not name the binding here.
    pub(crate) fn finish_assign_rename_source(
        &mut self,
        value: &Node,
        stored_val: &str,
        target_ty: &Option<Type>,
    ) {
        if let Node::Identifier(src) = value {
            if stored_val == self.resolve_binding_name(src)
                && !matches!(target_ty, Some(Type::Primitive(_)))
            {
                let src_resolved = self.resolve_binding_name(src);
                self.mark_consumed_binding(&src_resolved);
            }
        }
    }
}
