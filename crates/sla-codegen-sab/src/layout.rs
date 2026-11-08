//! D2 — layout adapter, mirroring `sab_codegen.zig` lines
//! 1590–1706 and 1776–1782: `primType`, `paramPrimType`,
//! `storagePrimType`, `addressableSlotPrimType`, `typeSize`,
//! `alignOffset`, the tuple/array layout queries, `structSize`,
//! `structDeclForType`, `fieldLayout`, `fieldType`,
//! `typeBaseName`, `firstGenericArg`, and the element-size
//! helpers (`elementSlotSize`, `elementLoadType`).
//!
//! The heavy facts live in sla-lowering (`abi_type_size`,
//! `align_aggregate_offset`, `abi_passes_as_pointer`,
//! `peel_borrow_pointer_type`); this layer adapts them to SAB
//! register types and caches struct-decl lookups, per the
//! decomposition note ("重活在 lowering，本层适配+缓存").
//!
//! SAB-D10口径: Zig's `tupleFieldLayout`/`arrayElementLayout`
//! collapse two distinct facts into one `null` — "no such field"
//! and "the field's type has no storage type" (`storagePrimType
//! catch return null`). The detailed entry points here return a
//! [`LayoutOutcome`] that keeps the two apart; the Zig-shaped
//! `Option` wrappers are kept for call sites that genuinely
//! consume the collapsed form, and are marked as such.

use std::collections::HashMap;

use sla_ast::{ArrayType, Primitive, StructDecl, TupleType, Type};
use sla_lowering::{
    abi_passes_as_pointer, abi_type_size, align_aggregate_offset, peel_borrow_pointer_type,
    CheckInfo,
};

use crate::error::{Error, Result};
use crate::ir::PrimType;
use crate::types::FieldLayout;

/// Why a layout query did not produce a layout (SAB-D10: kept
/// distinct instead of Zig's merged `null`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutMiss {
    /// The tuple index / array index / field name does not exist.
    NoSuchField,
    /// The field exists but its type has no storage prim type.
    UnmappableFieldType { type_name: String },
}

/// A layout query's detailed outcome (SAB-D10).
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutOutcome {
    Found(FieldLayout),
    Miss(LayoutMiss),
}

impl LayoutOutcome {
    /// The Zig collapsed form: both miss kinds become `None`.
    /// Only for call sites whose Zig original consumes `?FieldLayout`.
    pub fn collapsed(self) -> Option<FieldLayout> {
        match self {
            LayoutOutcome::Found(layout) => Some(layout),
            LayoutOutcome::Miss(_) => None,
        }
    }
}

/// Zig `userDefinedLocalName` (lowering_rules.zig:151): strip a
/// trailing `ns.` / `ns::` qualifier, if any.
pub fn user_defined_local_name(name: &str) -> &str {
    let dot = name.rfind('.').map(|i| i + 1).unwrap_or(0);
    let colon = name.rfind("::").map(|i| i + 2).unwrap_or(0);
    let start = dot.max(colon);
    if start == 0 || start >= name.len() {
        name
    } else {
        &name[start..]
    }
}

/// Zig `isUserDefinedNamed` (lowering_rules.zig, behind
/// `isFileType`): peeled type is a user-defined name with no
/// generic arguments.
fn is_user_defined_named(ty: &Type, name: &str) -> bool {
    let curr = peel_borrow_pointer_type(ty);
    match curr {
        Type::UserDefined(ud) => ud.name == name && ud.generics.is_empty(),
        _ => false,
    }
}

/// Zig `isFileType` (lowering_rules.zig:4889).
pub fn is_file_type(ty: &Type) -> bool {
    is_user_defined_named(ty, "File")
}

/// Zig `concreteTypeName` / `typeBaseName`
/// (lowering_rules.zig:5968/5974): the peeled user-defined name.
pub fn type_base_name(ty: &Type) -> Option<String> {
    let curr = peel_borrow_pointer_type(ty);
    match curr {
        Type::UserDefined(ud) => Some(ud.name.clone()),
        _ => None,
    }
}

/// Zig `firstGenericArg` (lowering_rules.zig:5978).
pub fn first_generic_arg(ty: &Type) -> Option<Type> {
    let curr = peel_borrow_pointer_type(ty);
    match curr {
        Type::UserDefined(ud) => ud.generics.first().cloned(),
        _ => None,
    }
}

/// Zig `userDefinedGenericInner`: the single generic argument of
/// a named user-defined type (`Vec<T>` etc.).
pub fn user_defined_generic_inner(ty: &Type, name: &str) -> Option<Type> {
    let curr = peel_borrow_pointer_type(ty);
    match curr {
        Type::UserDefined(ud) if ud.name == name && ud.generics.len() == 1 => {
            ud.generics.first().cloned()
        }
        _ => None,
    }
}

/// Zig `enumNameMatchesDecl` (lowering_rules.zig:4086): exact, or
/// the decl name extends the pattern with a `_` segment.
pub fn enum_name_matches_decl(pattern_name: &str, decl_name: &str) -> bool {
    if pattern_name == decl_name {
        return true;
    }
    if decl_name.len() <= pattern_name.len() {
        return false;
    }
    decl_name.starts_with(pattern_name) && decl_name.as_bytes()[pattern_name.len()] == b'_'
}

/// Zig `primType` (lines 1590–1611), primitive arm verbatim;
/// non-primitives go through the lowering facts exactly as in
/// Zig: File is a u64 handle, pointer-passed types are `ptr`,
/// everything else is `UnsupportedSabDirectFeature`.
pub fn prim_type(ty: &Type) -> Result<PrimType> {
    match ty {
        Type::Primitive(p) => Ok(match p {
            Primitive::I8 => PrimType::I8,
            Primitive::I16 => PrimType::I16,
            Primitive::I32 => PrimType::I32,
            Primitive::I64 | Primitive::Integer | Primitive::Isize => PrimType::I64,
            Primitive::U8 => PrimType::U8,
            Primitive::U16 => PrimType::U16,
            Primitive::U32 => PrimType::U32,
            Primitive::U64 | Primitive::Usize => PrimType::U64,
            Primitive::F32 => PrimType::F32,
            Primitive::F64 | Primitive::Float => PrimType::F64,
            Primitive::Boolean => PrimType::I1,
            Primitive::RawPtr => PrimType::Ptr,
            Primitive::VoidType => PrimType::Void,
            Primitive::VaList => PrimType::Ptr,
        }),
        _ => {
            if is_file_type(ty) {
                Ok(PrimType::U64)
            } else if abi_passes_as_pointer(ty) {
                Ok(PrimType::Ptr)
            } else {
                Err(Error::UnsupportedSabDirectFeature(format!(
                    "primType of non-primitive type {ty:?} [group D2 layout; Zig sab_codegen.zig:1590-1611]"
                )))
            }
        }
    }
}

/// Zig `paramPrimType`: raw pointers stay `ptr`, else `primType`.
pub fn param_prim_type(ty: &Type) -> Result<PrimType> {
    if matches!(ty, Type::Primitive(Primitive::RawPtr)) {
        return Ok(PrimType::Ptr);
    }
    prim_type(ty)
}

/// Zig `storagePrimType` (lines 1619–1624): aggregate storage
/// widens bool to a byte; raw pointers stay `ptr`.
pub fn storage_prim_type(ty: &Type) -> Result<PrimType> {
    if matches!(ty, Type::Primitive(Primitive::Boolean)) {
        return Ok(PrimType::U8);
    }
    if matches!(ty, Type::Primitive(Primitive::RawPtr)) {
        return Ok(PrimType::Ptr);
    }
    prim_type(ty)
}

/// Zig `addressableSlotPrimType`: an inferred type's slot is a
/// pointer; everything else stores at its storage type.
pub fn addressable_slot_prim_type(ty: &Type) -> Result<PrimType> {
    if matches!(ty, Type::Infer) {
        return Ok(PrimType::Ptr);
    }
    storage_prim_type(ty)
}

/// Zig `typeSize` = `lowering_rules.abiTypeSize`.
pub fn type_size(ty: &Type) -> usize {
    abi_type_size(ty)
}

/// Zig `alignOffset` = `lowering_rules.alignAggregateOffset`.
pub fn align_offset(offset: usize, size: usize) -> usize {
    align_aggregate_offset(offset, size)
}

/// Zig `tupleSize` = `lowering_rules.tupleAbiSize`.
pub fn tuple_size(tuple: &TupleType) -> usize {
    sla_lowering::tuple_abi_size(tuple)
}

/// Zig `lowering_rules.tupleFieldLayout` walk, detailed
/// (SAB-D10): offsets accumulate with per-element alignment.
pub fn tuple_field_layout_detailed(tuple: &TupleType, index: usize) -> LayoutOutcome {
    let mut offset = 0usize;
    for (i, elem_ty) in tuple.elems.iter().enumerate() {
        let size = abi_type_size(elem_ty);
        offset = align_aggregate_offset(offset, size);
        if i == index {
            return match storage_prim_type(elem_ty) {
                Ok(prim) => LayoutOutcome::Found(FieldLayout { offset, ty: prim }),
                Err(_) => LayoutOutcome::Miss(LayoutMiss::UnmappableFieldType {
                    type_name: format!("{elem_ty:?}"),
                }),
            };
        }
        offset += size;
    }
    LayoutOutcome::Miss(LayoutMiss::NoSuchField)
}

/// Zig `tupleFieldLayout` (line 1633), collapsed form.
pub fn tuple_field_layout(tuple: &TupleType, index: usize) -> Option<FieldLayout> {
    tuple_field_layout_detailed(tuple, index).collapsed()
}

/// Zig `lowering_rules.inlineArrayStride`: at least one byte.
pub fn array_stride(elem_ty: &Type) -> usize {
    abi_type_size(elem_ty).max(1)
}

/// Zig `lowering_rules.inlineArraySize`: at least one byte.
pub fn array_size(arr: &ArrayType) -> usize {
    (array_stride(&arr.elem) * arr.len).max(1)
}

/// Zig `arrayElementLayout` (line 1650), detailed (SAB-D10).
pub fn array_element_layout_detailed(arr: &ArrayType, index: usize) -> LayoutOutcome {
    if index >= arr.len {
        return LayoutOutcome::Miss(LayoutMiss::NoSuchField);
    }
    let offset = array_stride(&arr.elem) * index;
    match storage_prim_type(&arr.elem) {
        Ok(prim) => LayoutOutcome::Found(FieldLayout { offset, ty: prim }),
        Err(_) => LayoutOutcome::Miss(LayoutMiss::UnmappableFieldType {
            type_name: format!("{:?}", arr.elem),
        }),
    }
}

/// Zig `arrayElementLayout`, collapsed form.
pub fn array_element_layout(arr: &ArrayType, index: usize) -> Option<FieldLayout> {
    array_element_layout_detailed(arr, index).collapsed()
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

/// Zig `structSize` = `lowering_rules.structAbiSize`
/// (lowering_rules.zig:5514): opaque is 1; a union is its largest
/// field; otherwise the aligned field walk, at least 1.
pub fn struct_size(decl: &StructDecl) -> usize {
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

/// Zig `lowering_rules.structFieldLayout` (lowering_rules.zig:
/// 5554), detailed: unions place every field at offset 0.
pub fn struct_field_layout_detailed(decl: &StructDecl, name: &str) -> LayoutOutcome {
    if decl.is_union {
        for field in &decl.fields {
            if field.name == name {
                return match storage_prim_type(&field.ty) {
                    Ok(prim) => LayoutOutcome::Found(FieldLayout { offset: 0, ty: prim }),
                    Err(_) => LayoutOutcome::Miss(LayoutMiss::UnmappableFieldType {
                        type_name: format!("{:?}", field.ty),
                    }),
                };
            }
        }
        return LayoutOutcome::Miss(LayoutMiss::NoSuchField);
    }
    let mut offset = 0usize;
    for field in &decl.fields {
        let size = struct_field_storage_size(&field.ty);
        offset = align_aggregate_offset(offset, struct_field_alignment_size(&field.ty));
        if field.name == name {
            return match storage_prim_type(&field.ty) {
                Ok(prim) => LayoutOutcome::Found(FieldLayout { offset, ty: prim }),
                Err(_) => LayoutOutcome::Miss(LayoutMiss::UnmappableFieldType {
                    type_name: format!("{:?}", field.ty),
                }),
            };
        }
        offset += size;
    }
    LayoutOutcome::Miss(LayoutMiss::NoSuchField)
}

/// The layout adapter proper: struct-decl resolution over the
/// checker surface, with the Zig lookup order
/// (`structs` → alias cache → local-name `structs` → local-name
/// alias cache) and a name-keyed cache. The Rust `CheckInfo`
/// exposes one struct table, which serves both the direct and
/// the alias-cache slots (registered in docs/divergences/sab.md
/// as part of the D2 structural notes).
pub struct LayoutAdapter<'c> {
    check: &'c dyn CheckInfo,
    struct_decl_cache: HashMap<String, Option<StructDecl>>,
}

impl<'c> LayoutAdapter<'c> {
    pub fn new(check: &'c dyn CheckInfo) -> Self {
        LayoutAdapter {
            check,
            struct_decl_cache: HashMap::new(),
        }
    }

    /// Zig `structDeclForType` (lines 1711–1724).
    pub fn struct_decl_for_type(&mut self, ty: &Type) -> Option<StructDecl> {
        let curr = peel_borrow_pointer_type(ty);
        let ud = match curr {
            Type::UserDefined(ud) => ud,
            _ => return None,
        };
        let name = ud.name.clone();
        if let Some(cached) = self.struct_decl_cache.get(&name) {
            return cached.clone();
        }
        let resolved = self.resolve_struct_decl(&name);
        self.struct_decl_cache.insert(name, resolved.clone());
        resolved
    }

    fn resolve_struct_decl(&self, name: &str) -> Option<StructDecl> {
        if let Some(decl) = self.check.struct_decl(name) {
            return Some(decl.clone());
        }
        // Zig's alias_struct_cache slot: the CheckInfo struct
        // table is the same table here, so a second probe would
        // be identical; the local-name fallback is the next step.
        let local = user_defined_local_name(name);
        if local == name {
            return None;
        }
        self.check.struct_decl(local).cloned()
    }

    /// Zig `fieldLayout` (line 1870): opaque decls and missing
    /// fields are loud `UnsupportedSabDirectFeature`, never a
    /// collapsed null — this is the emitter-facing entry point.
    pub fn field_layout(&mut self, ty: &Type, name: &str) -> Result<FieldLayout> {
        let decl = self.struct_decl_for_type(ty).ok_or_else(|| {
            Error::UnsupportedSabDirectFeature(format!(
                "fieldLayout: no struct decl for {ty:?} [group D2 layout; Zig sab_codegen.zig:1870-1876]"
            ))
        })?;
        if decl.is_opaque {
            return Err(Error::UnsupportedSabDirectFeature(format!(
                "fieldLayout on opaque struct `{}` [group D2 layout; Zig sab_codegen.zig:1870-1876]",
                decl.name
            )));
        }
        match struct_field_layout_detailed(&decl, name) {
            LayoutOutcome::Found(layout) => Ok(layout),
            LayoutOutcome::Miss(miss) => Err(Error::UnsupportedSabDirectFeature(format!(
                "fieldLayout: {miss:?} for field `{name}` [group D2 layout; Zig sab_codegen.zig:1870-1876]"
            ))),
        }
    }

    /// Zig `fieldType` (line 1878): the declared field type, or
    /// `None` when the struct or field is unknown (Zig's nullable
    /// return, kept as-is here).
    pub fn field_type(&mut self, ty: &Type, name: &str) -> Option<Type> {
        let decl = self.struct_decl_for_type(ty)?;
        decl.fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| (*f.ty).clone())
    }

    /// Zig `structSize` over a resolved decl.
    pub fn struct_size_of(&mut self, ty: &Type) -> Option<usize> {
        self.struct_decl_for_type(ty).map(|decl| struct_size(&decl))
    }
}

/// Zig `vecElementType` (lowering_rules.zig:4698).
pub fn vec_element_type(ty: &Type) -> Option<Type> {
    user_defined_generic_inner(ty, "Vec")
}

/// Zig `vecElementSlotSize`: at least one word.
pub fn vec_element_slot_size(elem_ty: &Type) -> usize {
    abi_type_size(elem_ty).max(8)
}

/// Zig `elementSlotSize` (line 1896): Vec elements use the Vec
/// slot size; any other generic type uses its first argument's
/// plain size; non-generic types fall back to one word (8).
pub fn element_slot_size(ty: &Type) -> usize {
    if let Some(elem_ty) = vec_element_type(ty) {
        return vec_element_slot_size(&elem_ty);
    }
    if let Some(elem_ty) = first_generic_arg(ty) {
        return type_size(&elem_ty);
    }
    8
}

/// Zig `elementLoadType` (line 1940): the first generic
/// argument's prim type; no generic argument is loud.
pub fn element_load_type(ty: &Type) -> Result<PrimType> {
    let elem_ty = first_generic_arg(ty).ok_or_else(|| {
        Error::UnsupportedSabDirectFeature(format!(
            "elementLoadType of non-generic type {ty:?} [group D2 layout; Zig sab_codegen.zig:1940-1944]"
        ))
    })?;
    prim_type(&elem_ty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{Field, UserDefined};
    use sla_lowering::MapCheckInfo;

    fn prim(p: Primitive) -> Type {
        Type::Primitive(p)
    }

    fn ud(name: &str, generics: Vec<Type>) -> Type {
        Type::UserDefined(UserDefined {
            name: name.to_string(),
            generics,
        })
    }

    fn field(name: &str, ty: Type) -> Field {
        Field {
            name: name.to_string(),
            ty: Box::new(ty),
        }
    }

    #[test]
    fn prim_and_storage_types() {
        assert_eq!(prim_type(&prim(Primitive::Boolean)).unwrap(), PrimType::I1);
        assert_eq!(storage_prim_type(&prim(Primitive::Boolean)).unwrap(), PrimType::U8);
        assert_eq!(prim_type(&prim(Primitive::Integer)).unwrap(), PrimType::I64);
        assert_eq!(prim_type(&prim(Primitive::Float)).unwrap(), PrimType::F64);
        assert_eq!(prim_type(&ud("File", vec![])).unwrap(), PrimType::U64);
        assert_eq!(prim_type(&ud("Vec", vec![prim(Primitive::I32)])).unwrap(), PrimType::Ptr);
        assert_eq!(
            addressable_slot_prim_type(&Type::Infer).unwrap(),
            PrimType::Ptr
        );
        // Tuples pass as pointers; closure types are in neither
        // Zig's nor sla-lowering's pointer set and stay loud.
        assert!(prim_type(&Type::Tuple(TupleType { elems: vec![] })).is_ok());
    }

    #[test]
    fn tuple_and_array_layouts_distinguish_misses() {
        let tuple = TupleType {
            elems: vec![prim(Primitive::U8), prim(Primitive::I64)],
        };
        let layout = tuple_field_layout(&tuple, 1).expect("field 1 exists");
        assert_eq!(layout.offset, 8);
        assert_eq!(layout.ty, PrimType::I64);
        assert_eq!(
            tuple_field_layout_detailed(&tuple, 5),
            LayoutOutcome::Miss(LayoutMiss::NoSuchField)
        );
        let arr = ArrayType {
            elem: Box::new(prim(Primitive::U16)),
            len: 4,
        };
        assert_eq!(array_size(&arr), 8);
        let elem = array_element_layout(&arr, 3).expect("elem 3 exists");
        assert_eq!(elem.offset, 6);
        assert!(array_element_layout(&arr, 4).is_none());
    }

    #[test]
    fn struct_layout_and_adapter() {
        let decl = StructDecl {
            name: "Pair".to_string(),
            derives: vec![],
            generics: vec![],
            fields: vec![
                field("tag", prim(Primitive::U8)),
                field("value", prim(Primitive::I64)),
                field("flag", prim(Primitive::Boolean)),
            ],
            is_union: false,
            is_opaque: false,
        };
        // Zig structAbiSize returns the raw walk offset (no
        // tail padding): 1 + pad + 8 + 1 = 17.
        assert_eq!(struct_size(&decl), 17);
        let value = struct_field_layout_detailed(&decl, "value");
        assert_eq!(
            value,
            LayoutOutcome::Found(FieldLayout {
                offset: 8,
                ty: PrimType::I64
            })
        );
        // bool stores as u8 in aggregate storage.
        let flag = struct_field_layout_detailed(&decl, "flag");
        assert_eq!(
            flag,
            LayoutOutcome::Found(FieldLayout {
                offset: 16,
                ty: PrimType::U8
            })
        );

        let mut check = MapCheckInfo::new();
        check.add_struct(decl);
        let mut adapter = LayoutAdapter::new(&check);
        let ty = ud("Pair", vec![]);
        assert_eq!(adapter.field_layout(&ty, "value").unwrap().offset, 8);
        assert!(adapter.field_layout(&ty, "nope").is_err());
        assert_eq!(
            adapter.field_type(&ty, "flag"),
            Some(prim(Primitive::Boolean))
        );
        // Qualified name resolves through the local-name fallback.
        let qualified = ud("ns.Pair", vec![]);
        assert!(adapter.struct_decl_for_type(&qualified).is_some());
    }

    #[test]
    fn element_sizes() {
        let vec_i32 = ud("Vec", vec![prim(Primitive::I32)]);
        assert_eq!(element_slot_size(&vec_i32), 8);
        let vec_i64 = ud("Vec", vec![prim(Primitive::I64)]);
        assert_eq!(element_load_type(&vec_i64).unwrap(), PrimType::I64);
        let plain = prim(Primitive::I32);
        assert_eq!(element_slot_size(&plain), 8);
        assert!(element_load_type(&plain).is_err());
        assert_eq!(type_base_name(&vec_i32).as_deref(), Some("Vec"));
        assert_eq!(user_defined_local_name("a::b::Vec"), "Vec");
        assert!(enum_name_matches_decl("Opt", "Opt_Some"));
        assert!(!enum_name_matches_decl("Opt", "Option"));
    }
}
