//! Driver type/op helpers, extracted verbatim from `driver.rs`
//! at phase-4 integration (file-length rule: every .rs <= 700
//! lines). Pure move, no behavior change: `primType` /
//! `abiReturnPrimType` / `returnCapForType` /
//! `scalarBinaryOpKind` mirrors and the group-attributed
//! `unsupported` error constructor (SAB-D15). `driver.rs`
//! re-exports every item, so existing `crate::driver::...`
//! paths are unchanged.

use sla_ast::{Primitive, Type};
use sla_lowering::{
    abi_cap_kind_from_type, abi_passes_as_pointer, peel_borrow_pointer_type, AbiCapKind,
    ScalarBinaryOp,
};

use crate::error::{Error, Result};
use crate::ir::{CapPrefix, OpKind, PrimType};

/// A loud, group-attributed unsupported error (SAB-D15: the reason
/// string always travels with the error).
pub(crate) fn unsupported(what: &str, group: &str, zig_ref: &str) -> Error {
    Error::UnsupportedSabDirectFeature(format!(
        "{what} [group {group}; Zig sab_codegen.zig:{zig_ref}]"
    ))
}

/// Zig `primType` (lines 1569–1591), primitive arm. Non-primitive
/// types route through the same lowering facts as Zig's
/// `primType` else arm: a `File` user-defined type is a u64
/// handle, and every `abiPassesAsPointer` type (pointer / borrow
/// / fn-ptr / closure / user-defined / tuple / array / future)
/// is `ptr`; anything else stays loud, exactly as in Zig.
pub(crate) fn prim_type(ty: &Type) -> Result<PrimType> {
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
            if is_driver_file_type(ty) {
                Ok(PrimType::U64)
            } else if abi_passes_as_pointer(ty) {
                Ok(PrimType::Ptr)
            } else {
                Err(unsupported(
                    "non-primitive type has no primType in the driver subset",
                    "D2 layout",
                    "1569-1591",
                ))
            }
        }
    }
}

/// Zig `isFileType` via `isUserDefinedNamed` (lowering_rules.zig):
/// peel borrow/pointer layers, then a user-defined `File` with
/// no generic arguments. Local copy of layout.rs's predicate so
/// the driver subset does not depend on the D2 adapter module.
fn is_driver_file_type(ty: &Type) -> bool {
    match peel_borrow_pointer_type(ty) {
        Type::UserDefined(ud) => ud.name == "File" && ud.generics.is_empty(),
        _ => false,
    }
}

/// Zig `abiReturnPrimType`: bool returns widen to i32.
pub(crate) fn abi_return_prim_type(ty: &Type) -> Result<PrimType> {
    if matches!(ty, Type::Primitive(Primitive::Boolean)) {
        return Ok(PrimType::I32);
    }
    prim_type(ty)
}

/// Zig `returnCapForType` (lines 5628–5635) over sla-lowering's
/// `abiCapKindFromType`.
pub(crate) fn return_cap_for_type(ty: &Type) -> Option<CapPrefix> {
    match abi_cap_kind_from_type(ty) {
        AbiCapKind::Borrow => Some(CapPrefix::Borrow),
        AbiCapKind::Move => Some(CapPrefix::Move),
        AbiCapKind::Raw => Some(CapPrefix::Raw),
        AbiCapKind::None => None,
    }
}

/// Zig `scalarBinaryOpKind` (lines 2349–2380): lowering plan ->
/// SAB op kind.
pub(crate) fn scalar_binary_op_kind(plan: ScalarBinaryOp) -> OpKind {
    match plan {
        ScalarBinaryOp::Add => OpKind::Add,
        ScalarBinaryOp::Sub => OpKind::Sub,
        ScalarBinaryOp::Mul => OpKind::Mul,
        ScalarBinaryOp::SDiv => OpKind::Sdiv,
        ScalarBinaryOp::UDiv => OpKind::Udiv,
        ScalarBinaryOp::SRem => OpKind::Srem,
        ScalarBinaryOp::URem => OpKind::Urem,
        ScalarBinaryOp::BitAnd | ScalarBinaryOp::LogicalAnd => OpKind::And,
        ScalarBinaryOp::BitOr | ScalarBinaryOp::LogicalOr => OpKind::Or,
        ScalarBinaryOp::BitXor => OpKind::Xor,
        ScalarBinaryOp::Shl => OpKind::Shl,
        ScalarBinaryOp::LShr => OpKind::Lshr,
        ScalarBinaryOp::AShr => OpKind::Ashr,
        ScalarBinaryOp::Eq => OpKind::Eq,
        ScalarBinaryOp::Ne => OpKind::Ne,
        ScalarBinaryOp::SLt => OpKind::Slt,
        ScalarBinaryOp::SLe => OpKind::Sle,
        ScalarBinaryOp::SGt => OpKind::Sgt,
        ScalarBinaryOp::SGe => OpKind::Sge,
        ScalarBinaryOp::ULt => OpKind::Ult,
        ScalarBinaryOp::ULe => OpKind::Ule,
        ScalarBinaryOp::UGt => OpKind::Ugt,
        ScalarBinaryOp::UGe => OpKind::Uge,
        ScalarBinaryOp::FAdd => OpKind::Fadd,
        ScalarBinaryOp::FSub => OpKind::Fsub,
        ScalarBinaryOp::FMul => OpKind::Fmul,
        ScalarBinaryOp::FDiv => OpKind::Fdiv,
        ScalarBinaryOp::FCmpEq => OpKind::FcmpEq,
        ScalarBinaryOp::FCmpNe => OpKind::FcmpNe,
        ScalarBinaryOp::FCmpLt => OpKind::FcmpLt,
        ScalarBinaryOp::FCmpLe => OpKind::FcmpLe,
        ScalarBinaryOp::FCmpGt => OpKind::FcmpGt,
        ScalarBinaryOp::FCmpGe => OpKind::FcmpGe,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{ArrayType, TupleType, UserDefined};

    fn prim(p: Primitive) -> Type {
        Type::Primitive(p)
    }

    fn ud(name: &str, generics: Vec<Type>) -> Type {
        Type::UserDefined(UserDefined {
            name: name.to_string(),
            generics,
        })
    }

    #[test]
    fn driver_prim_type_matches_zig_else_arm() {
        // File is a u64 handle; user-defined / tuple / array pass
        // as pointers, exactly like Zig primType's else arm.
        assert_eq!(prim_type(&ud("File", vec![])).unwrap(), PrimType::U64);
        assert_eq!(
            prim_type(&ud("Vec", vec![prim(Primitive::I32)])).unwrap(),
            PrimType::Ptr
        );
        assert_eq!(
            prim_type(&Type::Tuple(TupleType {
                elems: vec![prim(Primitive::U8)]
            }))
            .unwrap(),
            PrimType::Ptr
        );
        assert_eq!(
            prim_type(&Type::Array(ArrayType {
                elem: Box::new(prim(Primitive::U8)),
                len: 4,
            }))
            .unwrap(),
            PrimType::Ptr
        );
        // A generic File is not the handle type and stays loud,
        // as do inferred types.
        assert!(prim_type(&ud("File", vec![prim(Primitive::U8)])).is_err());
        assert!(prim_type(&Type::Infer).is_err());
    }
}

