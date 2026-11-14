//! Expression-check leaf helpers split out of check_expr.rs
//! for the 700-line file cap (bodies unchanged): borrow/pointer
//! peeling, cast admissibility, and index result typing.

use sla_ast::exprs::Literal;
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type, UserDefined};

use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    btree_map_types, hash_map_types, is_numeric_type, is_string_type, type_desc, types_equal,
};

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

/// DEF-RUST-STRKEY-V2: mirror of Zig `refineStringLiteralArg`
/// (type_checker.zig:5432, DEF-IDX-B). A string literal types
/// as raw_ptr standalone, but against a String (or still
/// inferred) key slot it denotes a String, so key-slot checks
/// admit literal keys on String-keyed containers. BTree key
/// slots are deliberately NOT refined (their key model is
/// raw_ptr/slice by design, btreeSetKeySupported).
pub fn refine_string_literal_key(arg: &Node, arg_ty: Type, slot_ty: &Type) -> Type {
    if matches!(arg, Node::Literal(Literal::StringVal(_)))
        && (matches!(slot_ty, Type::Infer) || is_string_type(slot_ty))
    {
        Type::UserDefined(UserDefined {
            name: "String".to_string(),
            generics: Vec::new(),
        })
    } else {
        arg_ty
    }
}

/// DEF-RUST-STRKEY-V2: the map arms of Zig's `index_expr`
/// typing (type_checker.zig:8651). A HashMap target keys the
/// index by its declared key type (with the string-literal
/// refinement) and yields the VALUE type; a BTreeMap target
/// keys by its key type without refinement and likewise
/// yields the value type. Returns `None` for non-map
/// targets, whose integer gate stays in the caller
/// (check_expr.rs) exactly as before. Before this arm
/// existed every index expression fell into the integer
/// gate, so `m[k]` on any map was rejected at check — and
/// the generic result helper returned the KEY type
/// (generics[0]) as the result type for the map shapes
/// that did slip through (an int index on a String map
/// surfaced only as a downstream let-type mismatch).
pub fn map_index_result_type(
    target: &Type,
    index_node: &Node,
    index_ty: &Type,
) -> Option<CheckResult<Type>> {
    if let Some((key_ty, value_ty)) = hash_map_types(target) {
        let refined = refine_string_literal_key(index_node, index_ty.clone(), &key_ty);
        if !types_equal(&key_ty, &refined) {
            return Some(Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "TypeMismatch: HashMap index expects key {}, actual {}",
                    type_desc(&key_ty),
                    type_desc(&refined)
                ),
            )));
        }
        return Some(Ok(value_ty));
    }
    if let Some((key_ty, value_ty)) = btree_map_types(target) {
        if !types_equal(&key_ty, index_ty) {
            return Some(Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "TypeMismatch: BTreeMap index expects key {}, actual {}",
                    type_desc(&key_ty),
                    type_desc(index_ty)
                ),
            )));
        }
        return Some(Ok(value_ty));
    }
    None
}
