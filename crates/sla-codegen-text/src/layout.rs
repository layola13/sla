//! Aggregate layout facts for the text emitter, mirroring
//! `lowering_rules.zig`'s fieldLayout family (structFieldLayout,
//! tupleFieldLayout, enumFieldLayout, the *AbiSize walks). The
//! same walks exist in the SAB crate's layout adapter over the
//! same sla-lowering facts; this copy is local to the text crate
//! so the frozen SAB route is not touched. All sizes come from
//! `sla_lowering::abi_type_size` / `align_aggregate_offset`.

use sla_ast::{ArrayType, EnumDecl, EnumVariant, StructDecl, Type};
use sla_lowering::{abi_type_size, align_aggregate_offset};

/// Zig `lowering_rules.inlineArrayStride`: at least one byte.
pub(crate) fn array_stride(elem: &Type) -> usize {
    abi_type_size(elem).max(1)
}

/// Zig `lowering_rules.inlineArraySize`: at least one byte.
pub(crate) fn array_size(arr: &ArrayType) -> usize {
    (array_stride(&arr.elem) * arr.len).max(1)
}

/// Zig `lowering_rules.structFieldStorageSize`: fixed arrays
/// store inline (at least the pointer-sized ABI size).
fn struct_field_storage_size(ty: &Type) -> usize {
    match ty {
        Type::Array(arr) => array_size(arr).max(abi_type_size(ty)),
        _ => abi_type_size(ty),
    }
}

/// Zig `lowering_rules.structFieldAlignmentSize`.
fn struct_field_alignment_size(ty: &Type) -> usize {
    match ty {
        Type::Array(arr) => abi_type_size(&arr.elem),
        _ => abi_type_size(ty),
    }
}

/// Zig `structSize` = `lowering_rules.structAbiSize`: opaque is
/// 1; a union is its largest field; otherwise the aligned field
/// walk, at least 1.
pub(crate) fn struct_size(decl: &StructDecl) -> usize {
    if decl.is_opaque {
        return 1;
    }
    if decl.is_union {
        let max_size = decl
            .fields
            .iter()
            .map(|f| abi_type_size(&f.ty))
            .max()
            .unwrap_or(0);
        return max_size.max(1);
    }
    let mut offset = 0usize;
    for field in &decl.fields {
        offset = align_aggregate_offset(offset, struct_field_alignment_size(&field.ty));
        offset += struct_field_storage_size(&field.ty);
    }
    offset.max(1)
}

/// Zig `lowering_rules.structFieldLayout`: (offset, field type);
/// unions place every field at offset 0.
pub(crate) fn struct_field_layout(decl: &StructDecl, name: &str) -> Option<(usize, Type)> {
    if decl.is_union {
        return decl
            .fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| (0, (*f.ty).clone()));
    }
    let mut offset = 0usize;
    for field in &decl.fields {
        offset = align_aggregate_offset(offset, struct_field_alignment_size(&field.ty));
        if field.name == name {
            return Some((offset, (*field.ty).clone()));
        }
        offset += struct_field_storage_size(&field.ty);
    }
    None
}

/// Zig `lowering_rules.tupleFieldLayout`: (offset, element type).
pub(crate) fn tuple_field_layout(elems: &[Type], index: usize) -> Option<(usize, Type)> {
    let mut offset = 0usize;
    for (i, elem_ty) in elems.iter().enumerate() {
        let size = abi_type_size(elem_ty);
        offset = align_aggregate_offset(offset, size);
        if i == index {
            return Some((offset, elem_ty.clone()));
        }
        offset += size;
    }
    None
}

/// Enum payloads start after the i64 discriminant.
pub(crate) const ENUM_PAYLOAD_OFFSET: usize = 8;

/// Zig `lowering_rules.enumVariantValue`: explicit discriminant,
/// else C-like auto-increment.
pub(crate) fn enum_variant_value(decl: &EnumDecl, name: &str) -> Option<i64> {
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

/// Zig `lowering_rules.enumFieldLayout`: (offset, field type).
pub(crate) fn enum_field_layout(variant: &EnumVariant, name: &str) -> Option<(usize, Type)> {
    let mut offset = ENUM_PAYLOAD_OFFSET;
    for field in &variant.fields {
        let size = abi_type_size(&field.ty);
        offset = align_aggregate_offset(offset, size);
        if field.name == name {
            return Some((offset, (*field.ty).clone()));
        }
        offset += size;
    }
    None
}

/// Zig `lowering_rules.enumAbiSize`.
pub(crate) fn enum_abi_size(decl: &EnumDecl) -> usize {
    let mut max_payload = 0usize;
    for variant in &decl.variants {
        let mut offset = ENUM_PAYLOAD_OFFSET;
        for field in &variant.fields {
            let size = abi_type_size(&field.ty);
            offset = align_aggregate_offset(offset, size);
            offset += size;
        }
        max_payload = max_payload.max(offset - ENUM_PAYLOAD_OFFSET);
    }
    (ENUM_PAYLOAD_OFFSET + max_payload).max(ENUM_PAYLOAD_OFFSET)
}
