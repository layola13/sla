//! I group — sequential aggregate literals: tuples, arrays,
//! repeat arrays, and enum literals.
//!
//! Faithful port of the current-head Zig (reference tree
//! /content/sa_all/sa_plugin_sla @ 2cc12ea8, sab_codegen.zig):
//! `genTupleLiteral` (22133), `genArrayLiteral` (22180),
//! `genRepeatArrayLiteral` (22239), `emitRepeatArrayFill` (10983),
//! `genEnumLiteral` (21472), with the lowering_rules enum helpers
//! (lowering_rules.zig:5579–5652: `enum_tag_offset`,
//! `enumVariantValue`, `enumVariant`, `enumFieldLayout`,
//! `enumAbiSize`, `enumLiteralFieldValue`).
//!
//! The repeat-fill bulk arms and the enum `Option::None` arm emit
//! std call bodies / macro fragments through the dep-ensuring and
//! fragment machines (D4/F groups). Dep ensuring inside a function
//! body queues into `pending_std_deps` in Zig as it does here
//! (SAB-D26 queue semantics); the fragment arm stops loudly and
//! its post-merge home (`FragmentMachine`, SAB-F delivery) is
//! registered in the I-group REPORT.

use sla_ast::{ArrayType, EnumDecl, EnumLiteral, Node, TupleLiteral, Type};
use sla_lowering::abi_type_size;

use crate::copy_facts::CopyFacts;
use crate::driver::{unsupported, Driver};
use crate::error::{Error, Result};
use crate::ir::PrimType;
use crate::layout::{self, align_offset};
use crate::types::PendingStdDep;

/// Zig `bulk_array_threshold` (sab_codegen.zig:25).
const BULK_ARRAY_THRESHOLD: usize = 32;
/// Zig `lowering_rules.enum_tag_offset` (5579).
const ENUM_TAG_OFFSET: usize = 0;
/// Zig `lowering_rules.enum_payload_offset` (5580).
const ENUM_PAYLOAD_OFFSET: usize = 8;

/// Zig `lowering_rules.enumVariantValue` (5596): explicit
/// discriminant, else C-like auto-increment.
fn enum_variant_value(decl: &EnumDecl, name: &str) -> Option<i64> {
    let mut next_auto: i64 = 0;
    for variant in &decl.variants {
        let value = variant.discriminant.unwrap_or(next_auto);
        if variant.name == name {
            return Some(value);
        }
        next_auto = value + 1;
    }
    None
}

/// Zig `lowering_rules.enumVariant` (5607).
fn enum_variant<'d>(decl: &'d EnumDecl, name: &str) -> Option<&'d sla_ast::EnumVariant> {
    decl.variants.iter().find(|v| v.name == name)
}

/// Zig `lowering_rules.enumFieldLayout` (5617), in the port's D2
/// layout form (offset + storage prim).
fn enum_field_layout(variant: &sla_ast::EnumVariant, name: &str) -> Option<(usize, Type)> {
    let mut offset = ENUM_PAYLOAD_OFFSET;
    for field in &variant.fields {
        let size = abi_type_size(&field.ty);
        offset = align_offset(offset, size);
        if field.name == name {
            return Some((offset, (*field.ty).clone()));
        }
        offset += size;
    }
    None
}

/// Zig `lowering_rules.enumAbiSize` (5631).
fn enum_abi_size(decl: &EnumDecl) -> usize {
    let mut max_payload = 0usize;
    for variant in &decl.variants {
        let mut offset = ENUM_PAYLOAD_OFFSET;
        for field in &variant.fields {
            let size = abi_type_size(&field.ty);
            offset = align_offset(offset, size);
            offset += size;
        }
        max_payload = max_payload.max(offset - ENUM_PAYLOAD_OFFSET);
    }
    (ENUM_PAYLOAD_OFFSET + max_payload).max(ENUM_PAYLOAD_OFFSET)
}

impl<'c> Driver<'c> {
    /// Zig `ensureStdDeps` in-function queueing half (D4): the dep
    /// is recorded once, in request order.
    pub(crate) fn queue_std_dep(&mut self, import_path: &str, dep: &str) {
        let entry = PendingStdDep {
            import_path: import_path.to_string(),
            dep: dep.to_string(),
        };
        if !self.state.program.pending_std_deps.contains(&entry) {
            self.state.program.pending_std_deps.push(entry);
        }
    }

    /// Zig `genTupleLiteral` (22133).
    pub(crate) fn gen_tuple_literal(
        &mut self,
        expr: &Node,
        lit: &TupleLiteral,
    ) -> Result<u32> {
        if lit.elements.is_empty() {
            return Err(Error::UnsupportedSabDirectFeature(
                "empty tuple literal".to_string(),
            ));
        }
        let tuple_ty: sla_ast::TupleType = match self.check.expr_type(expr) {
            Some(Type::Tuple(tuple)) => tuple.clone(),
            Some(_) => {
                return Err(Error::UnsupportedSabDirectFeature(
                    "tuple literal with a non-tuple checker type".to_string(),
                ))
            }
            None => {
                let mut elems = Vec::with_capacity(lit.elements.len());
                for elem in &lit.elements {
                    elems.push(
                        self.check
                            .expr_type(elem)
                            .cloned()
                            .ok_or_else(|| {
                                Error::MissingType(
                                    "tuple element type (genTupleLiteral fallback)".to_string(),
                                )
                            })?,
                    );
                }
                sla_ast::TupleType { elems }
            }
        };
        let dst = self.b().new_tmp();
        self.emit_alloc(dst, layout::tuple_size(&tuple_ty));
        for (idx, elem) in lit.elements.iter().enumerate() {
            let elem_layout = layout::tuple_field_layout(&tuple_ty, idx)
                .ok_or(Error::UnsupportedSabDirectFeature(
                    "tuple element has no layout".to_string(),
                ))?;
            let value = self.gen_expr(elem)?;
            self.emit_store(dst, elem_layout.offset, value, elem_layout.ty);
            if self.state.frame.current_expr_result_escapes && self.is_param_reg(value) {
                self.emit_consumed_marker(value);
                self.mark_consumed(value);
            }
            let elem_ty = &tuple_ty.elems[idx];
            let is_copy = {
                let mut facts = CopyFacts::new(self.check);
                facts.type_is_copy_value(elem_ty)
            };
            if is_copy {
                self.release_stored_expr_result_if_needed(elem, value, elem_ty)?;
            } else {
                self.mark_stored_value_moved_if_needed(elem, elem_ty);
                if !self.is_local_reg(value) {
                    self.emit_move(value);
                }
            }
        }
        Ok(dst)
    }

    /// Zig `genArrayLiteral` (22180).
    pub(crate) fn gen_array_literal(
        &mut self,
        expr: &Node,
        lit: &sla_ast::ArrayLiteral,
    ) -> Result<u32> {
        let arr_ty = self
            .check
            .expr_type(expr)
            .cloned()
            .ok_or_else(|| Error::MissingType("array literal type".to_string()))?;
        let Type::Array(arr) = &arr_ty else {
            return Err(Error::UnsupportedSabDirectFeature(
                "array literal with a non-array checker type".to_string(),
            ));
        };
        if arr.len != lit.elements.len() {
            return Err(Error::UnsupportedSabDirectFeature(
                "array literal length does not match its checker type".to_string(),
            ));
        }
        let arr = arr.clone();
        let dst = self.b().new_tmp();
        self.emit_alloc(dst, layout::array_size(&arr));
        for (idx, elem) in lit.elements.iter().enumerate() {
            let elem_layout = layout::array_element_layout(&arr, idx)
                .ok_or(Error::UnsupportedSabDirectFeature(
                    "array element has no layout".to_string(),
                ))?;
            let value = self.gen_expr(elem)?;
            self.emit_store(dst, elem_layout.offset, value, elem_layout.ty);
            let is_copy = {
                let mut facts = CopyFacts::new(self.check);
                facts.type_is_copy_value(&arr.elem)
            };
            if is_copy {
                self.release_stored_expr_result_if_needed(elem, value, &arr.elem)?;
            } else {
                self.emit_move(value);
            }
        }
        Ok(dst)
    }

    /// Zig `genRepeatArrayLiteral` (22239).
    pub(crate) fn gen_repeat_array_literal(
        &mut self,
        expr: &Node,
        lit: &sla_ast::RepeatArrayLiteral,
    ) -> Result<u32> {
        let arr_ty = self
            .check
            .expr_type(expr)
            .cloned()
            .ok_or_else(|| Error::MissingType("repeat array literal type".to_string()))?;
        let Type::Array(arr) = &arr_ty else {
            return Err(Error::UnsupportedSabDirectFeature(
                "repeat array literal with a non-array checker type".to_string(),
            ));
        };
        if arr.len != lit.len {
            return Err(Error::UnsupportedSabDirectFeature(
                "repeat array length does not match its checker type".to_string(),
            ));
        }
        let arr = arr.clone();
        // Zig: `_ = try primType(arr_ty.array.elem);` — an
        // unmappable element type is a hard error before any
        // emission.
        layout::prim_type(&arr.elem)?;
        let dst = self.b().new_tmp();
        self.emit_alloc(dst, layout::array_size(&arr));
        // [value; N] evaluates `value` once per element for
        // composite elements (a single shared register would alias
        // every slot to one heap object); plain scalars may share
        // one evaluation.
        let elem_is_scalar = matches!(
            &*arr.elem,
            Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_)
        );
        if elem_is_scalar {
            let value = self.gen_expr(&lit.value)?;
            if !self.emit_repeat_array_fill(dst, &arr, value)? {
                for idx in 0..lit.len {
                    let elem_layout = layout::array_element_layout(&arr, idx)
                        .ok_or(Error::UnsupportedSabDirectFeature(
                            "repeat array element has no layout".to_string(),
                        ))?;
                    self.emit_store(dst, elem_layout.offset, value, elem_layout.ty);
                }
            }
            self.release_stored_expr_result_if_needed(&lit.value, value, &arr.elem)?;
        } else {
            for idx in 0..lit.len {
                let elem_layout = layout::array_element_layout(&arr, idx)
                    .ok_or(Error::UnsupportedSabDirectFeature(
                        "repeat array element has no layout".to_string(),
                    ))?;
                let slot_value = self.gen_expr(&lit.value)?;
                self.emit_store(dst, elem_layout.offset, slot_value, elem_layout.ty);
                self.release_stored_expr_result_if_needed(&lit.value, slot_value, &arr.elem)?;
            }
        }
        Ok(dst)
    }

    /// Zig `emitRepeatArrayFill` (10983): `true` when a bulk fill
    /// replaced the per-element stores.
    pub(crate) fn emit_repeat_array_fill(
        &mut self,
        dst: u32,
        arr: &ArrayType,
        value: u32,
    ) -> Result<bool> {
        if matches!(&*arr.elem, Type::Primitive(sla_ast::Primitive::U8)) {
            self.queue_std_dep("sa_std/core/mem.sa", "sa_mem_set");
            let dst_name = self.b().symbol_name(dst).to_string();
            let value_name = self.b().symbol_name(value).to_string();
            self.b().emit_call(
                None,
                &format!("@sa_mem_set(&{dst_name}, {value_name}, {})", arr.len),
            );
            return Ok(true);
        }
        if arr.len <= BULK_ARRAY_THRESHOLD {
            return Ok(false);
        }
        if layout::array_element_layout(arr, 0).is_none() {
            return Ok(false);
        }
        // Bulk fill: store once, then double the filled prefix with
        // bulk copies until the array is covered.
        let elem_size = layout::array_stride(&arr.elem);
        let layout0 = layout::array_element_layout(arr, 0).unwrap();
        self.emit_store(dst, layout0.offset, value, layout0.ty);
        self.queue_std_dep("sa_std/core/mem.sa", "sa_mem_copy_nonoverlapping");
        let dst_name = self.b().symbol_name(dst).to_string();
        let mut filled = 1usize;
        while filled < arr.len {
            let chunk = filled.min(arr.len - filled);
            let ptr = self.b().new_tmp();
            self.emit_ptr_add_imm(ptr, dst, filled * elem_size);
            let ptr_name = self.b().symbol_name(ptr).to_string();
            self.b().emit_call(
                None,
                &format!(
                    "@sa_mem_copy_nonoverlapping(&{ptr_name}, &{dst_name}, {})",
                    chunk * elem_size
                ),
            );
            self.emit_release(ptr)?;
            filled += chunk;
        }
        Ok(true)
    }

    /// Zig `genEnumLiteral` (21472). The `Option::None` arm emits
    /// the OPTION_NEW_NONE std macro fragment (F-group machine;
    /// post-merge `FragmentMachine`) and stops loudly here.
    pub(crate) fn gen_enum_literal(&mut self, lit: &EnumLiteral) -> Result<u32> {
        if lit.enum_name == "Option" && lit.variant_name == "None" && lit.fields.is_empty() {
            return Err(unsupported(
                "Option::None literal (OPTION_NEW_NONE fragment)",
                "F fragments",
                "21478-21486",
            ));
        }
        let decl = self
            .enums
            .get(&lit.enum_name)
            .cloned()
            .ok_or(Error::UnsupportedSabDirectFeature(
                "enum literal with no enum decl in the program".to_string(),
            ))?;
        let tag = enum_variant_value(&decl, &lit.variant_name)
            .ok_or(Error::UnsupportedSabDirectFeature(
                "enum literal variant not in decl".to_string(),
            ))?;
        let variant = enum_variant(&decl, &lit.variant_name)
            .ok_or(Error::UnsupportedSabDirectFeature(
                "enum literal variant not in decl".to_string(),
            ))?
            .clone();

        let dst = self.b().new_tmp();
        self.emit_alloc(dst, enum_abi_size(&decl));
        let tag_reg = self.b().new_tmp();
        self.b().emit_assign_imm(tag_reg, tag);
        self.emit_store(dst, ENUM_TAG_OFFSET, tag_reg, PrimType::I64);
        self.emit_release(tag_reg)?;

        for field in &variant.fields {
            let value = lit
                .fields
                .iter()
                .find(|f| f.name == field.name)
                .map(|f| &*f.value)
                .ok_or(Error::UnsupportedSabDirectFeature(
                    "enum literal is missing a variant field value".to_string(),
                ))?;
            let (offset, field_ty) = enum_field_layout(&variant, &field.name)
                .ok_or(Error::UnsupportedSabDirectFeature(
                    "enum variant field has no layout".to_string(),
                ))?;
            let prim = layout::storage_prim_type(&field_ty).map_err(|_| {
                Error::UnsupportedSabDirectFeature(
                    "enum variant field has no storage type".to_string(),
                )
            })?;
            let value_reg = self.gen_expr(value)?;
            self.emit_store(dst, offset, value_reg, prim);
            self.release_stored_expr_result_if_needed(value, value_reg, &field_ty)?;
        }
        Ok(dst)
    }
}
