//! ABI type facts: register type names, sizes, parameter prefixes and
//! Copy-ness leaves, mirrored from lowering_rules.zig. These are the
//! Y-SHARE facts both SA-text and SAB emitters must agree on; the Zig
//! comments say so explicitly at `sharedAbiParamPrefix`.

use sla_ast::{Primitive, Type};

/// Integer primitive set behind lowering_rules.zig `isIntegerPrimitive`.
pub fn is_integer_primitive(p: Primitive) -> bool {
    matches!(
        p,
        Primitive::I8
            | Primitive::I16
            | Primitive::I32
            | Primitive::I64
            | Primitive::Isize
            | Primitive::U8
            | Primitive::U16
            | Primitive::U32
            | Primitive::U64
            | Primitive::Usize
            | Primitive::Integer
    )
}

/// Float primitive set behind lowering_rules.zig `isFloatPrimitive`.
pub fn is_float_primitive(p: Primitive) -> bool {
    matches!(p, Primitive::F32 | Primitive::F64 | Primitive::Float)
}

/// lowering_rules.zig:5474 `isFloatType`.
pub fn is_float_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(p) if is_float_primitive(*p))
}

/// lowering_rules.zig:5478 `isUnsignedIntegerType`.
pub fn is_unsigned_integer_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(p) if matches!(p,
        Primitive::U8 | Primitive::U16 | Primitive::U32 | Primitive::U64 | Primitive::Usize))
}

/// lowering_rules.zig:5487 `isAnyIntegerType`.
pub fn is_any_integer_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(p) if is_integer_primitive(*p))
}

/// lowering_rules.zig:5505 `isNumericType`.
pub fn is_numeric_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(p) if is_integer_primitive(*p) || is_float_primitive(*p))
}

/// lowering_rules.zig:5497 `isI32LikeType`.
pub fn is_i32_like_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(p) if matches!(p, Primitive::I32 | Primitive::Integer))
}

/// lowering_rules.zig:5500 `zeroLiteralForType`.
pub fn zero_literal_for_type(ty: &Type) -> &'static str {
    if is_float_type(ty) {
        "0.0"
    } else {
        "0"
    }
}

/// lowering_rules.zig:1109 `isVoidType`.
pub fn is_void_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(Primitive::VoidType))
}

/// lowering_rules.zig:1112 `abiTypeString`.
pub fn abi_type_string(ty: &Type) -> &'static str {
    match ty {
        Type::Primitive(p) => match p {
            Primitive::Boolean => "u8",
            Primitive::I8 => "i8",
            Primitive::I16 => "i16",
            Primitive::I32 => "i32",
            Primitive::I64 => "i64",
            Primitive::Isize => "i64",
            Primitive::U8 => "u8",
            Primitive::U16 => "u16",
            Primitive::U32 => "u32",
            Primitive::U64 => "u64",
            Primitive::Usize => "u64",
            Primitive::F32 => "f32",
            Primitive::F64 => "f64",
            Primitive::Integer => "i64",
            Primitive::Float => "f64",
            Primitive::RawPtr => "ptr",
            Primitive::VoidType => "ptr",
        },
        Type::Array(_) | Type::Tuple(_) => "ptr",
        _ => "ptr",
    }
}

/// lowering_rules.zig:1135 `abiReturnTypeString`.
pub fn abi_return_type_string(ty: &Type) -> &'static str {
    match ty {
        Type::Borrow(_) => "&ptr",
        _ => abi_type_string(ty),
    }
}

/// lowering_rules.zig:1188 `abiParamTypeString`.
pub fn abi_param_type_string(is_borrow: bool, is_move: bool, ty: &Type) -> &'static str {
    if is_borrow || is_move {
        "ptr"
    } else {
        abi_type_string(ty)
    }
}

/// lowering_rules.zig:1193 `abiParamNeedsBorrowArg`.
pub fn abi_param_needs_borrow_arg(is_borrow: bool, ty: &Type) -> bool {
    is_borrow || matches!(ty, Type::Borrow(_))
}

/// Shared pure ABI capability kind (lowering_rules.zig `AbiCapKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiCapKind {
    None,
    Borrow,
    Move,
    Raw,
}

/// lowering_rules.zig:1149 `abiCapKindFromType`.
pub fn abi_cap_kind_from_type(ty: &Type) -> AbiCapKind {
    if matches!(ty, Type::Borrow(_)) {
        AbiCapKind::Borrow
    } else {
        AbiCapKind::None
    }
}

/// lowering_rules.zig:1154 `abiCapKindFromRawTypeString`.
pub fn abi_cap_kind_from_raw_type_string(raw: &str) -> AbiCapKind {
    let name = raw.trim_matches(|c| matches!(c, ' ' | '\t' | '\r'));
    if name.is_empty() {
        return AbiCapKind::None;
    }
    match name.as_bytes()[0] {
        b'&' => AbiCapKind::Borrow,
        b'^' => AbiCapKind::Move,
        b'*' => AbiCapKind::Raw,
        _ => AbiCapKind::None,
    }
}

/// lowering_rules.zig:1176 `abiCallArgCapKind`.
pub fn abi_call_arg_cap_kind(is_borrow: bool, is_move: bool) -> AbiCapKind {
    if is_borrow {
        return AbiCapKind::Borrow;
    }
    if is_move {
        return AbiCapKind::Move;
    }
    AbiCapKind::None
}

/// lowering_rules.zig:1200 `sharedAbiParamPrefix` (Y-SHARE: `&` = borrow,
/// `^` = move; copy scalars and borrow-likes get no prefix).
pub fn shared_abi_param_prefix(
    is_borrow: bool,
    is_move: bool,
    ty_is_borrow: bool,
    is_by_value_raw_ptr: bool,
    is_copy_value: bool,
    is_borrow_like: bool,
) -> &'static str {
    if is_borrow || ty_is_borrow {
        return "&";
    }
    if is_by_value_raw_ptr {
        return "";
    }
    if is_move || (!is_copy_value && !is_borrow_like) {
        return "^";
    }
    ""
}

/// lowering_rules.zig:1218 `sharedAbiParamCap`.
pub fn shared_abi_param_cap(
    is_borrow: bool,
    is_move: bool,
    ty_is_borrow: bool,
    is_by_value_raw_ptr: bool,
    is_copy_value: bool,
    is_borrow_like: bool,
) -> AbiCapKind {
    let prefix = shared_abi_param_prefix(
        is_borrow,
        is_move,
        ty_is_borrow,
        is_by_value_raw_ptr,
        is_copy_value,
        is_borrow_like,
    );
    if prefix.is_empty() {
        return AbiCapKind::None;
    }
    match prefix.as_bytes()[0] {
        b'&' => AbiCapKind::Borrow,
        b'^' => AbiCapKind::Move,
        _ => AbiCapKind::None,
    }
}

/// lowering_rules.zig:6125 `isBorrowLikeType`.
pub fn is_borrow_like_type(ty: &Type) -> bool {
    match ty {
        Type::Borrow(_) => true,
        Type::Primitive(p) => *p == Primitive::RawPtr,
        _ => false,
    }
}

/// lowering_rules.zig:6006 `isRawPtrAliasType`.
pub fn is_raw_ptr_alias_type(ty: &Type) -> bool {
    matches!(ty, Type::Primitive(Primitive::RawPtr))
}

/// lowering_rules.zig:5995 `isPointerCarrierCastType`.
pub fn is_pointer_carrier_cast_type(ty: &Type) -> bool {
    match ty {
        Type::Pointer(_) | Type::Borrow(_) => true,
        Type::Primitive(p) => *p == Primitive::RawPtr,
        Type::UserDefined(ud) => matches!(
            ud.name.as_str(),
            "AtomicI32" | "AtomicUsize" | "RawWaker" | "Waker" | "LocalWaker" | "Wake"
        ),
        _ => false,
    }
}

/// Single-layer pointer/borrow pointee (lowering_rules.zig
/// `pointerOrBorrowPointee`).
pub fn pointer_or_borrow_pointee(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Pointer(p) => Some(p),
        Type::Borrow(b) => Some(b),
        _ => None,
    }
}

/// Peel borrow/pointer layers (codegen's `peelBorrowPointerType` helper
/// thins to this shape).
pub fn peel_borrow_pointer_type(ty: &Type) -> &Type {
    let mut cur = ty;
    loop {
        match cur {
            Type::Pointer(p) => cur = p,
            Type::Borrow(b) => cur = b,
            _ => return cur,
        }
    }
}

/// lowering_rules.zig:6237 `rawPointerValueType` (private there).
fn raw_pointer_value_type(ty: &Type) -> bool {
    match ty {
        Type::Primitive(p) => *p == Primitive::RawPtr,
        Type::Pointer(_) => true,
        _ => false,
    }
}

/// lowering_rules.zig:6245 `byValueRawPointerParam`.
pub fn by_value_raw_pointer_param(param: &sla_ast::Param) -> bool {
    if param.is_borrow || param.is_move {
        return false;
    }
    raw_pointer_value_type(&param.ty)
}

/// lowering_rules.zig:6510 `primitiveIsCopyValue`.
pub fn primitive_is_copy_value(p: Primitive) -> bool {
    p != Primitive::RawPtr && p != Primitive::VoidType
}

/// lowering_rules.zig:6515 `typeIsCopyValueLeaf`: non-recursive Copy
/// facts; `None` where struct tables are needed.
pub fn type_is_copy_value_leaf(ty: &Type) -> Option<bool> {
    match ty {
        Type::Primitive(p) => Some(primitive_is_copy_value(*p)),
        Type::FnPtr(_) => Some(true),
        Type::Pointer(_) | Type::Borrow(_) => Some(false),
        _ => None,
    }
}

/// lowering_rules.zig:3851 `alignAggregateOffset`: only 8-byte elements
/// align (to 8); every other size leaves the offset untouched.
pub fn align_aggregate_offset(offset: usize, size: usize) -> usize {
    if size == 8 {
        (offset + 7) & !7usize
    } else {
        offset
    }
}

/// lowering_rules.zig:3787 `abiTypeSize`.
pub fn abi_type_size(ty: &Type) -> usize {
    match ty {
        Type::Primitive(p) => match p {
            Primitive::Boolean | Primitive::U8 | Primitive::I8 => 1,
            Primitive::U16 | Primitive::I16 => 2,
            Primitive::U32 | Primitive::I32 | Primitive::F32 => 4,
            Primitive::U64
            | Primitive::I64
            | Primitive::Usize
            | Primitive::Isize
            | Primitive::F64 => 8,
            Primitive::Integer | Primitive::Float => 8,
            Primitive::RawPtr | Primitive::VoidType => 8,
        },
        Type::Array(_) => 8,
        Type::Tuple(tuple) => tuple_abi_size(tuple),
        _ => 8,
    }
}

/// lowering_rules.zig:3856 `tupleAbiSize`.
pub fn tuple_abi_size(tuple: &sla_ast::TupleType) -> usize {
    let mut offset: usize = 0;
    for elem_ty in &tuple.elems {
        let size = abi_type_size(elem_ty);
        offset = align_aggregate_offset(offset, size);
        offset += size;
    }
    offset.max(1)
}

/// lowering_rules.zig:3830 `abiPassesAsPointer`.
pub fn abi_passes_as_pointer(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Pointer(_)
            | Type::Borrow(_)
            | Type::FnPtr(_)
            | Type::UserDefined(_)
            | Type::Tuple(_)
            | Type::Array(_)
            | Type::Future(_)
    )
}

/// Plan for borrowed-binding storage (lowering_rules.zig
/// `BorrowedBindingStoragePlan`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorrowedBindingStoragePlan {
    pub materialize_stack_slot: bool,
}

/// lowering_rules.zig:683 `planBorrowedBindingStorage`.
pub fn plan_borrowed_binding_storage(
    binding_is_borrowed: bool,
    ty: &Type,
) -> BorrowedBindingStoragePlan {
    BorrowedBindingStoragePlan {
        materialize_stack_slot: binding_is_borrowed && matches!(ty, Type::Primitive(_)),
    }
}
