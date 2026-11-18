//! Expression-check leaf helpers split out of check_expr.rs
//! for the 700-line file cap (bodies unchanged): borrow/pointer
//! peeling, cast admissibility, and index result typing.

use sla_ast::exprs::Literal;
use sla_ast::node::Node;
use sla_ast::types::{Primitive, Type, UserDefined};

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    btree_map_types, hash_map_types, is_numeric_type, is_string_type, node_key_of, type_desc,
    types_equal,
};

/// Deref peeling, mirroring the Zig `deref_expr` arm
/// (type_checker.zig): the smart-pointer / guard inner types are
/// tried FIRST — `userDefinedGenericInner` peels any borrow/pointer
/// wrappers before matching the name, so `*item` with
/// `item: &Box<T>` yields `T`, not `Box<T>` — and only when no
/// smart pointer matches does a single pointer/borrow peel apply
/// (`pointerOrBorrowPointee`). The previous single-layer form
/// stopped at `Box<T>` for a borrowed Box, so `&*item` typed as
/// `&Box<T>` and borrow-typed bindings rejected it
/// (test_unit_borrow_temp_release_order).
pub fn peel_pointer(ty: &Type) -> Option<Type> {
    let mut cur = ty;
    loop {
        match cur {
            Type::Pointer(inner) | Type::Borrow(inner) => cur = inner,
            _ => break,
        }
    }
    if let Type::UserDefined(u) = cur {
        if matches!(
            u.name.as_str(),
            "Rc" | "Arc" | "Box" | "MutexGuard" | "RwLockReadGuard" | "RwLockWriteGuard"
        ) && u.generics.len() == 1
        {
            return Some(u.generics[0].clone());
        }
    }
    match ty {
        Type::Pointer(inner) | Type::Borrow(inner) => Some((**inner).clone()),
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
    // Zig `isPointerCarrierCastType` (lowering_rules.zig): pointer,
    // borrow, the raw_ptr primitive, and the atomic/waker
    // user-defined carriers.
    let carrier = |t: &Type| {
        matches!(t, Type::Pointer(_) | Type::Borrow(_))
            || matches!(t, Type::Primitive(Primitive::RawPtr))
            || matches!(t, Type::UserDefined(ud) if matches!(
                ud.name.as_str(),
                "AtomicI32" | "AtomicUsize" | "RawWaker" | "Waker" | "LocalWaker" | "Wake"
            ))
    };
    if carrier(inner) && carrier(target) {
        return true;
    }
    // ZOZI-2 (Zig type_checker.zig cast arm): a pointer carrier
    // converts to any integer type and back (`handle as u64`,
    // `addr as *T`, `p as raw_ptr` chains through them), and a
    // function pointer converts to a raw_ptr / u64 / usize
    // primitive or to a pointer / borrow type. The previous form
    // admitted only carrier<->carrier, so the checker's own
    // ptr_int_casts corpus shape was a TypeMismatch.
    if carrier(inner) && crate::types_util::is_integer_type(target) {
        return true;
    }
    if crate::types_util::is_integer_type(inner) && carrier(target) {
        return true;
    }
    if matches!(inner, Type::FnPtr(_)) {
        return matches!(target, Type::Pointer(_) | Type::Borrow(_))
            || matches!(
                target,
                Type::Primitive(
                    Primitive::RawPtr | Primitive::U64 | Primitive::Usize
                )
            );
    }
    false
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

impl Checker {
    /// DEF-RUST-STRPTR-CHECKER: mirror of Zig
    /// `refineStringLiteralForSlot` (type_checker.zig:5444), the
    /// value-flow companion to `refineStringLiteralArg`. When a
    /// string literal flows into a value slot (let / const /
    /// assign / return / tail) whose slot type is String or still
    /// inferred, the literal denotes an owned String: refine its
    /// type and RECORD the refined type on the node, so codegen
    /// materializes an owned String handle instead of the
    /// literal's Slice view. Without this the standalone literal
    /// type (raw_ptr) met the slot check head-on and every
    /// `let s: String = "hello"` was a TypeMismatch, where Zig
    /// accepts and materializes a StringBuf.
    pub fn refine_string_literal_for_slot(
        &mut self,
        node: &Node,
        ty: Type,
        slot_ty: &Type,
    ) -> Type {
        let refined = refine_string_literal_key(node, ty.clone(), slot_ty);
        if refined != ty {
            self.expr_types.insert(node_key_of(node), refined.clone());
        }
        refined
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
