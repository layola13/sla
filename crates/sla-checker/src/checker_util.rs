//! Checker free-function helpers split out of checker.rs for
//! the 700-line file cap (bodies unchanged): the extern return
//! type table, borrow-type construction, panic-builtin names.

use crate::types_util;
use sla_ast::types::{Primitive, Type};

/// Map an `.sai` ABI return spelling to a type (Zig
/// `setTypeFromAbiReturn`). Unknown spellings degrade to `infer`,
/// which the Zig helper likewise leaves unset/wildcard in practice.
pub fn extern_ret_type(abi: &str) -> Type {
    use sla_ast::types::Primitive as P;
    match abi {
        "" | "void" => types_util::prim(P::VoidType),
        "ptr" | "*u8" | "*const u8" => types_util::prim(P::RawPtr),
        "bool" => types_util::prim(P::Boolean),
        "i8" => types_util::prim(P::I8),
        "i16" => types_util::prim(P::I16),
        "i32" => types_util::prim(P::I32),
        "i64" | "isize" => types_util::prim(P::I64),
        "u8" => types_util::prim(P::U8),
        "u16" => types_util::prim(P::U16),
        "u32" => types_util::prim(P::U32),
        "u64" | "usize" => types_util::prim(P::U64),
        "f32" => types_util::prim(P::F32),
        "f64" => types_util::prim(P::F64),
        _ => Type::Infer,
    }
}

/// Zig `makeBorrowType`: wrap a parameter type in a borrow unless it
/// already is one.
pub fn make_borrow_type(ty: &Type) -> Type {
    match ty {
        Type::Borrow(_) => ty.clone(),
        _ => Type::Borrow(Box::new(ty.clone())),
    }
}

/// Zig `isPanicBuiltinName` (lowering_rules.zig:4108).
pub fn is_panic_builtin_name(name: &str) -> bool {
    name == "panic" || name == "panic_msg"
}
