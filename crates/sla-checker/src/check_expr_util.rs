//! Expression-check leaf helpers split out of check_expr.rs
//! for the 700-line file cap (bodies unchanged): borrow/pointer
//! peeling, cast admissibility, and index result typing.

use sla_ast::types::{Primitive, Type};

use crate::types_util::{is_numeric_type, types_equal};

/// Deref peeling: pointers and borrows, plus the smart-pointer /
/// guard user types Zig peels by name (Rc / Arc / Box / guards).
pub fn peel_pointer(ty: &Type) -> Option<Type> {
    match ty {
        Type::Pointer(inner) | Type::Borrow(inner) => Some((**inner).clone()),
        Type::UserDefined(u) => {
            if matches!(
                u.name.as_str(),
                "Rc" | "Arc" | "Box" | "MutexGuard" | "RwLockReadGuard" | "RwLockWriteGuard"
            ) && !u.generics.is_empty()
            {
                Some(u.generics[0].clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn peel_borrow_pointer(ty: &Type) -> &Type {
    match ty {
        Type::Pointer(inner) | Type::Borrow(inner) => inner,
        other => other,
    }
}

pub fn cast_allowed(inner: &Type, target: &Type) -> bool {
    if matches!(inner, Type::Infer) || matches!(target, Type::Infer) {
        return true;
    }
    if types_equal(inner, target) {
        return true;
    }
    if is_numeric_type(inner) && is_numeric_type(target) {
        return true;
    }
    let carrier = |t: &Type| {
        matches!(t, Type::Pointer(_) | Type::Borrow(_))
            || matches!(t, Type::Primitive(Primitive::RawPtr))
    };
    carrier(inner) && carrier(target)
}

pub fn index_result_type(target: &Type) -> Option<Type> {
    match target {
        Type::Array(a) => Some((*a.elem).clone()),
        Type::Pointer(inner) => Some((**inner).clone()),
        // R-3: indexing through a borrow indexes the borrowed
        // value (borrow-origin views are `&[u8]` and are indexed
        // directly in the E_res corpus).
        Type::Borrow(inner) => index_result_type(inner),
        Type::UserDefined(u) => {
            if u.name == "String" || u.name == "str" {
                Some(Type::Primitive(Primitive::U8))
            } else if !u.generics.is_empty() {
                Some(u.generics[0].clone())
            } else {
                None
            }
        }
        _ => None,
    }
}
