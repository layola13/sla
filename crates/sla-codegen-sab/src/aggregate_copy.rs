//! I group — aggregate value copies: `genCopyValue`,
//! `genShallowCopyCallArgValue`, the in-group arms of
//! `genDeepCloneValue`, `copyArrayValueInto`, and
//! `storeArrayValueIntoStructField`.
//!
//! Split from `aggregate.rs` for the 700-line discipline; the
//! two files are one decomposition unit (see `aggregate.rs`
//! for the Zig line map). The fragment-backed clone arms
//! (String/Vec/GrowVec/Box/Rc/Arc) stop loudly in
//! `gen_deep_clone_value` with their coordinates.

use sla_ast::Type;

use crate::copy_facts::CopyFacts;
use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::layout::{self, LayoutAdapter};

impl<'c> Driver<'c> {
    /// Zig `genCopyValue` (22032): deep copy of a Copy-derive
    /// struct value into a fresh allocation, field by field.
    pub(crate) fn gen_copy_value(&mut self, source: u32, ty: &Type) -> Result<u32> {
        let mut layouts = LayoutAdapter::new(self.check);
        let decl = layouts
            .struct_decl_for_type(ty)
            .ok_or(Error::UnsupportedSabDirectFeature(
                "copy value of a type with no struct decl".to_string(),
            ))?;
        let has_copy_derive = {
            let mut facts = CopyFacts::new(self.check);
            facts.type_has_copy_derive(ty)
        };
        if !has_copy_derive || decl.is_opaque || decl.is_union {
            return Err(Error::UnsupportedSabDirectFeature(
                "copy value requires a non-opaque, non-union Copy-derive struct".to_string(),
            ));
        }
        let dst = self.b().new_tmp();
        self.emit_alloc(dst, layout::struct_size(&decl));
        for field in &decl.fields {
            let field_layout = layouts.field_layout(ty, &field.name)?;
            if let Type::Array(arr) = &*field.ty {
                self.copy_array_value_into(
                    dst,
                    field_layout.offset,
                    source,
                    field_layout.offset,
                    arr,
                    true,
                )?;
                continue;
            }
            let field_reg = self.b().new_tmp();
            self.emit_load(field_reg, source, field_layout.offset, field_layout.ty);
            if layouts.struct_decl_for_type(&field.ty).is_some() {
                let copied_field = self.gen_copy_value(field_reg, &field.ty)?;
                self.emit_store(dst, field_layout.offset, copied_field, field_layout.ty);
                self.emit_consumed_marker(copied_field);
                self.emit_release(field_reg)?;
            } else {
                self.emit_store(dst, field_layout.offset, field_reg, field_layout.ty);
                self.emit_consumed_marker(field_reg);
            }
        }
        Ok(dst)
    }

    /// Zig `genShallowCopyCallArgValue` (22058), with the
    /// `genDeepCloneValue` (7663) dispatch inlined per field: the
    /// fragment-backed clone arms stop loudly; the array/tuple/
    /// struct arms recurse inside this group.
    pub(crate) fn gen_shallow_copy_call_arg_value(
        &mut self,
        source: u32,
        ty: &Type,
    ) -> Result<u32> {
        let mut layouts = LayoutAdapter::new(self.check);
        let decl = layouts
            .struct_decl_for_type(ty)
            .ok_or(Error::UnsupportedSabDirectFeature(
                "shallow copy of a type with no struct decl".to_string(),
            ))?;
        let shallow = {
            let mut facts = CopyFacts::new(self.check);
            facts.type_is_shallow_copy_call_arg_value(ty, 0)
        };
        if !shallow || decl.is_opaque || decl.is_union {
            return Err(Error::UnsupportedSabDirectFeature(
                "shallow copy requires a non-opaque, non-union shallow-copy struct".to_string(),
            ));
        }
        let dst = self.b().new_tmp();
        self.emit_alloc(dst, layout::struct_size(&decl));
        for field in &decl.fields {
            let field_layout = layouts.field_layout(ty, &field.name)?;
            if let Type::Array(arr) = &*field.ty {
                self.copy_array_value_into(
                    dst,
                    field_layout.offset,
                    source,
                    field_layout.offset,
                    arr,
                    false,
                )?;
                continue;
            }
            let field_reg = self.b().new_tmp();
            self.emit_load(field_reg, source, field_layout.offset, field_layout.ty);
            if let Some(cloned) = self.gen_deep_clone_value(field_reg, &field.ty)? {
                self.emit_store(dst, field_layout.offset, cloned, field_layout.ty);
                self.emit_consumed_marker(cloned);
                self.emit_release(field_reg)?;
            } else if shallow_field_should_recurse(
                layouts.struct_decl_for_type(&field.ty).is_some(),
                &field.ty,
            ) {
                let copied_field = self.gen_shallow_copy_call_arg_value(field_reg, &field.ty)?;
                self.emit_store(dst, field_layout.offset, copied_field, field_layout.ty);
                self.emit_consumed_marker(copied_field);
                self.emit_release(field_reg)?;
            } else {
                self.emit_store(dst, field_layout.offset, field_reg, field_layout.ty);
                self.emit_consumed_marker(field_reg);
            }
        }
        Ok(dst)
    }

    /// Zig `genDeepCloneValue` (7663): `None` mirrors Zig's null
    /// (no clone applies; the caller falls through). The String /
    /// Vec / GrowVec / Box / Rc / Arc arms are fragment-backed
    /// clones and stop loudly; array/tuple/struct arms stay in
    /// this group.
    pub(crate) fn gen_deep_clone_value(
        &mut self,
        source: u32,
        ty: &Type,
    ) -> Result<Option<u32>> {
        if let Type::UserDefined(ud) = ty {
            if matches!(
                ud.name.as_str(),
                "String" | "Vec" | "GrowVec" | "Box" | "Rc" | "Arc"
            ) {
                return Err(unsupported(
                    &format!("deep clone of `{}` (fragment-backed clone arm)", ud.name),
                    "F fragments / D surface (genDeepCloneValue)",
                    "7663-7676",
                ));
            }
        }
        if let Type::Array(arr) = ty {
            // Zig genArrayCloneValue (7730).
            let dst = self.b().new_tmp();
            self.emit_alloc(dst, layout::array_size(arr));
            self.copy_array_value_into(dst, 0, source, 0, arr, false)?;
            return Ok(Some(dst));
        }
        if let Type::Tuple(tuple) = ty {
            // Zig genTupleCloneValue (7742).
            let dst = self.b().new_tmp();
            self.emit_alloc(dst, layout::tuple_size(tuple));
            for (idx, elem_ty) in tuple.elems.iter().enumerate() {
                let elem_layout = layout::tuple_field_layout(tuple, idx)
                    .ok_or(Error::UnsupportedSabDirectFeature(
                        "tuple clone element has no layout".to_string(),
                    ))?;
                let elem_reg = self.b().new_tmp();
                self.emit_load(elem_reg, source, elem_layout.offset, elem_layout.ty);
                if let Some(cloned) = self.gen_deep_clone_value(elem_reg, elem_ty)? {
                    self.emit_store(dst, elem_layout.offset, cloned, elem_layout.ty);
                    self.emit_consumed_marker(cloned);
                    self.emit_release(elem_reg)?;
                } else {
                    self.emit_store(dst, elem_layout.offset, elem_reg, elem_layout.ty);
                    self.emit_consumed_marker(elem_reg);
                }
            }
            return Ok(Some(dst));
        }
        let mut layouts = LayoutAdapter::new(self.check);
        if layouts.struct_decl_for_type(ty).is_some() {
            let shallow = {
                let mut facts = CopyFacts::new(self.check);
                facts.type_is_shallow_copy_call_arg_value(ty, 0)
            };
            if shallow && shallow_field_should_recurse(true, ty) {
                return Ok(Some(self.gen_shallow_copy_call_arg_value(source, ty)?));
            }
        }
        Ok(None)
    }

    /// Zig `copyArrayValueInto` (21986). The `deep == false`
    /// (record-copy) path routes each element through
    /// `gen_deep_clone_value`, exactly as Zig routes it through
    /// `genDeepCloneValue`.
    pub(crate) fn copy_array_value_into(
        &mut self,
        dst: u32,
        dst_offset: usize,
        source: u32,
        source_offset: usize,
        arr: &sla_ast::ArrayType,
        deep: bool,
    ) -> Result<()> {
        for idx in 0..arr.len {
            let elem_layout = layout::array_element_layout(arr, idx)
                .ok_or(Error::UnsupportedSabDirectFeature(
                    "array copy element has no layout".to_string(),
                ))?;
            let elem = &*arr.elem;
            if let Type::Array(inner) = elem {
                self.copy_array_value_into(
                    dst,
                    dst_offset + elem_layout.offset,
                    source,
                    source_offset + elem_layout.offset,
                    inner,
                    deep,
                )?;
            } else if !deep {
                let elem_reg = self.b().new_tmp();
                self.emit_load(elem_reg, source, source_offset + elem_layout.offset, elem_layout.ty);
                if let Some(cloned) = self.gen_deep_clone_value(elem_reg, elem)? {
                    self.emit_store(dst, dst_offset + elem_layout.offset, cloned, elem_layout.ty);
                    self.emit_consumed_marker(cloned);
                    self.emit_release(elem_reg)?;
                } else {
                    let mut layouts = LayoutAdapter::new(self.check);
                    if layouts.struct_decl_for_type(elem).is_some() {
                        let copied = self.gen_shallow_copy_call_arg_value(elem_reg, elem)?;
                        self.emit_store(
                            dst,
                            dst_offset + elem_layout.offset,
                            copied,
                            elem_layout.ty,
                        );
                        self.emit_consumed_marker(copied);
                        self.emit_release(elem_reg)?;
                    } else {
                        self.emit_store(
                            dst,
                            dst_offset + elem_layout.offset,
                            elem_reg,
                            elem_layout.ty,
                        );
                        self.emit_consumed_marker(elem_reg);
                    }
                }
            } else {
                let mut layouts = LayoutAdapter::new(self.check);
                if layouts.struct_decl_for_type(elem).is_some() {
                    let elem_reg = self.b().new_tmp();
                    self.emit_load(
                        elem_reg,
                        source,
                        source_offset + elem_layout.offset,
                        elem_layout.ty,
                    );
                    let copied = self.gen_copy_value(elem_reg, elem)?;
                    self.emit_store(dst, dst_offset + elem_layout.offset, copied, elem_layout.ty);
                    self.emit_consumed_marker(copied);
                    self.emit_release(elem_reg)?;
                } else {
                    let elem_reg = self.b().new_tmp();
                    self.emit_load(
                        elem_reg,
                        source,
                        source_offset + elem_layout.offset,
                        elem_layout.ty,
                    );
                    self.emit_store(dst, dst_offset + elem_layout.offset, elem_reg, elem_layout.ty);
                    self.emit_consumed_marker(elem_reg);
                }
            }
        }
        Ok(())
    }

    /// Zig `storeArrayValueIntoStructField` (22202). The bulk
    /// path (`arr.len > 32`, sab_codegen.zig:25 threshold) emits
    /// one `@sa_mem_copy_nonoverlapping` call body over the whole
    /// field region; the dep queues through D4's pending queue
    /// like every other std dep in this port (SAB-D26).
    pub(crate) fn store_array_value_into_struct_field(
        &mut self,
        dst: u32,
        field_offset: usize,
        arr: &sla_ast::ArrayType,
        value_reg: u32,
    ) -> Result<()> {
        const BULK_ARRAY_THRESHOLD: usize = 32; // sab_codegen.zig:25
        if arr.len > BULK_ARRAY_THRESHOLD && layout::array_element_layout(arr, 0).is_some() {
            self.queue_std_dep("sa_std/core/mem.sa", "sa_mem_copy_nonoverlapping");
            let mut dst_ptr = dst;
            if field_offset != 0 {
                dst_ptr = self.b().new_tmp();
                self.emit_ptr_add_imm(dst_ptr, dst, field_offset);
            }
            let dst_name = self.b().symbol_name(dst_ptr).to_string();
            let src_name = self.b().symbol_name(value_reg).to_string();
            self.b().emit_call(
                None,
                &format!(
                    "@sa_mem_copy_nonoverlapping(&{dst_name}, &{src_name}, {})",
                    layout::array_size(arr)
                ),
            );
            if dst_ptr != dst {
                self.emit_release(dst_ptr)?;
            }
            return Ok(());
        }
        for idx in 0..arr.len {
            let elem_layout = layout::array_element_layout(arr, idx)
                .ok_or(Error::UnsupportedSabDirectFeature(
                    "struct field array element has no layout".to_string(),
                ))?;
            let elem_reg = self.b().new_tmp();
            self.emit_load(elem_reg, value_reg, elem_layout.offset, elem_layout.ty);
            self.emit_store(
                dst,
                field_offset + elem_layout.offset,
                elem_reg,
                elem_layout.ty,
            );
            self.emit_consumed_marker(elem_reg);
            self.mark_consumed(elem_reg);
        }
        Ok(())
    }
}

/// Zig `lowering_rules.shallowCopyCallArgFieldShouldRecurse`
/// (6664), with its three predicates (`userDefinedStdOwnerIsNonCopy`,
/// `smartPointerType`, `isStdCollectionType`) collapsed to the
/// std-owner name set they enumerate; registered as SAB-D166.
fn shallow_field_should_recurse(field_has_struct_decl: bool, field_ty: &Type) -> bool {
    if !field_has_struct_decl {
        return false;
    }
    if let Type::UserDefined(ud) = field_ty {
        if matches!(
            ud.name.as_str(),
            "String"
                | "Vec"
                | "GrowVec"
                | "Box"
                | "Rc"
                | "Arc"
                | "HashMap"
                | "HashSet"
                | "BTreeMap"
                | "BTreeSet"
                | "VecDeque"
                | "ManuallyDrop"
        ) {
            return false;
        }
    }
    true
}
