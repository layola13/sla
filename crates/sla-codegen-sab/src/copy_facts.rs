//! D3 — copy facts, mirroring `sab_codegen.zig` lines
//! 1783–1883: `typeHasCopyDerive`, `typeIsCopyStruct`,
//! `typeIsCopyValue`, and `typeIsShallowCopyCallArgValue`.
//!
//! The leaf/base decisions are the `lowering_rules.zig` shared
//! facts (`typeHasCopyDeriveBase`, `typeIsCopyValueBase`,
//! `shallowCopyCallArgValueBase`, …). sla-lowering has ported
//! only the Copy-value leaf, so the remaining base facts are
//! ported locally here, line-for-line from the venue's current
//! head, and are marked for re-homing into sla-lowering when that
//! lane picks them up (the D-C3 trait-facts pattern).
//!
//! SAB-D05口径: Zig's derive cache writes are `catch {}` — a
//! failed cache insertion silently degrades to "recompute next
//! time", merging "derivation failed" with "no derive" in the
//! observable behaviour. Rust's `HashMap::insert` has no failure
//! channel to swallow, so the cache here is total; the Zig
//! swallowing is registered (SAB-D05), not reproduced — there is
//! no Rust construct that reproduces it without faking one.
//!
//! One consumption gap, registered as SAB-D27: Zig's shallow-copy
//! walk consults `tc.enums` (pure enums are shallow-copy); the
//! Rust `CheckInfo` surface exposes no enum table, so the enum
//! arm cannot fire here and enum-typed values fall through to
//! the struct-decl path (which finds no decl and answers false).
//! The trigger surface is exactly "user-defined enum type as a
//! nested call-arg value" — narrower than Zig, never wider.

use std::collections::HashMap;

use sla_ast::{Primitive, StructDecl, Type};
use sla_lowering::CheckInfo;

use crate::layout::{user_defined_generic_inner, LayoutAdapter};


/// Zig `primitiveIsCopyValue` (lowering_rules.zig:6802 in the
/// current head's numbering; the ported copy in sla-lowering's
/// `primitive_is_copy_value` agrees): everything except raw
/// pointers and void.
fn primitive_is_copy_value(p: Primitive) -> bool {
    !matches!(p, Primitive::RawPtr | Primitive::VoidType)
}

/// Zig `typeIsCopyValueLeaf` (lowering_rules.zig:6807).
fn type_is_copy_value_leaf(ty: &Type) -> Option<bool> {
    match ty {
        Type::Primitive(p) => Some(primitive_is_copy_value(*p)),
        Type::FnPtr(_) => Some(true),
        Type::Pointer(_) | Type::Borrow(_) => Some(false),
        _ => None,
    }
}

/// Zig `userDefinedStdOwnerIsNonCopy` (lowering_rules.zig:4972):
/// the fixed std-owner name list.
fn user_defined_std_owner_is_non_copy(ty: &Type) -> bool {
    let name = match ty {
        Type::UserDefined(ud) => ud.name.as_str(),
        _ => return false,
    };
    matches!(
        name,
        "Vec"
            | "GrowVec"
            | "VecDeque"
            | "String"
            | "Box"
            | "Rc"
            | "Arc"
            | "HashMap"
            | "BTreeMap"
            | "HashSet"
            | "BTreeSet"
            | "RefCell"
            | "Mutex"
            | "RwLock"
            | "JoinHandle"
    )
}

/// Zig `typeHasNamedDeriveBase` (lowering_rules.zig:6644 area):
/// primitives decided by `primitive_ok`; std owners are never
/// derived; other user-defined types defer (`None`); everything
/// else is false.
fn type_has_named_derive_base(ty: &Type, primitive_ok: bool) -> Option<bool> {
    match ty {
        Type::Primitive(_) => Some(primitive_ok),
        Type::UserDefined(_) => {
            if user_defined_std_owner_is_non_copy(ty) {
                Some(false)
            } else {
                None
            }
        }
        _ => Some(false),
    }
}

/// Zig `typeHasCopyDeriveBase` (lowering_rules.zig:6644).
fn type_has_copy_derive_base(ty: &Type) -> Option<bool> {
    let primitive_ok = match ty {
        Type::Primitive(p) => primitive_is_copy_value(*p),
        _ => false,
    };
    type_has_named_derive_base(ty, primitive_ok)
}

/// Zig `deriveNameMatches` / `structHasDerive`
/// (lowering_rules.zig:6–17): case-insensitive, with the
/// PartialEq→eq / PartialOrd→ord aliases.
pub fn struct_has_derive(decl: &StructDecl, name: &str) -> bool {
    decl.derives.iter().any(|derive| {
        derive.eq_ignore_ascii_case(name)
            || (name.eq_ignore_ascii_case("eq") && derive.eq_ignore_ascii_case("PartialEq"))
            || (name.eq_ignore_ascii_case("ord") && derive.eq_ignore_ascii_case("PartialOrd"))
    })
}

/// Zig `typeHasNamedDeriveStructGate`: derived, non-opaque,
/// non-union.
fn type_has_copy_derive_struct_gate(decl: &StructDecl) -> bool {
    struct_has_derive(decl, "copy") && !decl.is_opaque && !decl.is_union
}

/// Zig `typeIsCopyValueBase` (lowering_rules.zig:6619).
fn type_is_copy_value_base(ty: &Type) -> Option<bool> {
    match ty {
        Type::Primitive(_) | Type::FnPtr(_) => Some(type_is_copy_value_leaf(ty).unwrap_or(false)),
        Type::Tuple(_) | Type::Array(_) | Type::UserDefined(_) => None,
        _ => Some(false),
    }
}

/// Zig `isStdCollectionType` (lowering_rules.zig:6792).
fn is_std_collection_type(ty: &Type) -> bool {
    user_defined_generic_inner(ty, "Vec").is_some()
        || user_defined_generic_inner(ty, "GrowVec").is_some()
        || user_defined_generic_inner(ty, "VecDeque").is_some()
        || user_defined_generic_inner(ty, "HashSet").is_some()
        || user_defined_generic_inner(ty, "BTreeSet").is_some()
        || matches!(ty, Type::UserDefined(ud) if (ud.name == "HashMap" || ud.name == "BTreeMap") && ud.generics.len() == 2)
}

/// Zig `smartPointerType` presence (lowering_rules.zig:5028):
/// Box / Rc / Arc / RefCell generic inners.
fn smart_pointer_type_present(ty: &Type) -> bool {
    user_defined_generic_inner(ty, "Box").is_some()
        || user_defined_generic_inner(ty, "Rc").is_some()
        || user_defined_generic_inner(ty, "Arc").is_some()
        || user_defined_generic_inner(ty, "RefCell").is_some()
}

/// Zig `shallowCopyCallArgValueBase` (lowering_rules.zig:6525):
/// depth over 8 is a hard false (a decision, not a defer).
fn shallow_copy_call_arg_value_base(ty: &Type, depth: usize) -> Option<bool> {
    if depth > 8 {
        return Some(false);
    }
    if is_std_collection_type(ty) {
        return Some(depth > 0);
    }
    match ty {
        Type::Primitive(_) | Type::Pointer(_) | Type::Borrow(_) | Type::FnPtr(_) => Some(true),
        Type::Tuple(_) | Type::Array(_) | Type::UserDefined(_) => None,
        _ => Some(false),
    }
}

/// Zig `shallowCopyCallArgUserDefinedBase`
/// (lowering_rules.zig:6538).
fn shallow_copy_call_arg_user_defined_base(ty: &Type, depth: usize) -> Option<bool> {
    if user_defined_std_owner_is_non_copy(ty) || smart_pointer_type_present(ty) {
        return Some(depth > 0);
    }
    None
}

/// Zig `shallowCopyCallArgStructGate` (lowering_rules.zig:6545).
fn shallow_copy_call_arg_struct_gate(decl: &StructDecl) -> bool {
    !decl.is_opaque && !decl.is_union
}

/// The D3 fact engine: a layout adapter plus Zig's
/// `copy_value_cache` (program state in Zig; held here so the
/// cache's lifetime matches one `generate` run).
pub struct CopyFacts<'c> {
    layout: LayoutAdapter<'c>,
    /// Zig `copy_value_cache`: name → has-Copy-derive. Only
    /// generic-free user-defined names are cached, as in Zig.
    pub copy_value_cache: HashMap<String, bool>,
}

impl<'c> CopyFacts<'c> {
    pub fn new(check: &'c dyn CheckInfo) -> Self {
        CopyFacts {
            layout: LayoutAdapter::new(check),
            copy_value_cache: HashMap::new(),
        }
    }

    /// Zig `typeHasCopyDerive` (lines 1783–1808), recursion over
    /// struct fields included. The three cache writes are Zig's
    /// `catch {}` sites (SAB-D05) — see the module docs.
    pub fn type_has_copy_derive(&mut self, ty: &Type) -> bool {
        if let Some(decision) = type_has_copy_derive_base(ty) {
            return decision;
        }
        let ud = match ty {
            Type::UserDefined(ud) => ud,
            _ => return false,
        };
        let cache_name: Option<String> = if ud.generics.is_empty() {
            Some(ud.name.clone())
        } else {
            None
        };
        if let Some(name) = &cache_name {
            if let Some(cached) = self.copy_value_cache.get(name) {
                return *cached;
            }
        }
        let decl = match self.layout.struct_decl_for_type(ty) {
            Some(decl) => decl,
            None => {
                if let Some(name) = &cache_name {
                    self.copy_value_cache.insert(name.clone(), false);
                }
                return false;
            }
        };
        if !type_has_copy_derive_struct_gate(&decl) {
            if let Some(name) = &cache_name {
                self.copy_value_cache.insert(name.clone(), false);
            }
            return false;
        }
        let mut result = true;
        for field in &decl.fields {
            if !self.type_has_copy_derive(&field.ty) {
                result = false;
                break;
            }
        }
        if let Some(name) = &cache_name {
            self.copy_value_cache.insert(name.clone(), result);
        }
        result
    }

    /// Zig `typeIsCopyStruct` (line 1810) =
    /// `typeIsCopyStructFact(has_decl, has_copy_derive)`.
    pub fn type_is_copy_struct(&mut self, ty: &Type) -> bool {
        let has_decl = self.layout.struct_decl_for_type(ty).is_some();
        has_decl && self.type_has_copy_derive(ty)
    }

    /// Zig `typeIsCopyValue` (lines 1814–1832), including the
    /// goroutine-batch `Chan` handle rule at its head.
    pub fn type_is_copy_value(&mut self, ty: &Type) -> bool {
        if matches!(ty, Type::UserDefined(ud) if ud.name == "Chan") {
            return true;
        }
        if let Some(decision) = type_is_copy_value_base(ty) {
            return decision;
        }
        match ty {
            Type::UserDefined(_) => self.type_has_copy_derive(ty),
            Type::Tuple(tuple) => {
                let elems = tuple.elems.clone();
                elems.iter().all(|elem| self.type_is_copy_value(elem))
            }
            Type::Array(arr) => {
                let elem = (*arr.elem).clone();
                self.type_is_copy_value(&elem)
            }
            _ => false,
        }
    }

    /// Zig `typeIsShallowCopyCallArgValue` (lines 1834–1862),
    /// depth-recursive. The enum arm is the SAB-D27 gap (see the
    /// module docs): without an enum table it cannot fire.
    pub fn type_is_shallow_copy_call_arg_value(&mut self, ty: &Type, depth: usize) -> bool {
        if let Some(decision) = shallow_copy_call_arg_value_base(ty, depth) {
            return decision;
        }
        match ty {
            Type::Tuple(tuple) => {
                let elems = tuple.elems.clone();
                elems
                    .iter()
                    .all(|elem| self.type_is_shallow_copy_call_arg_value(elem, depth + 1))
            }
            Type::Array(arr) => {
                let elem = (*arr.elem).clone();
                self.type_is_shallow_copy_call_arg_value(&elem, depth + 1)
            }
            Type::UserDefined(_) => {
                if let Some(decision) = shallow_copy_call_arg_user_defined_base(ty, depth) {
                    return decision;
                }
                // Zig: pure enums are POD-like (`tc.enums`
                // membership) — SAB-D27: no enum table on
                // CheckInfo, so this arm cannot fire in the port.
                let decl = match self.layout.struct_decl_for_type(ty) {
                    Some(decl) => decl,
                    None => return false,
                };
                if !shallow_copy_call_arg_struct_gate(&decl) {
                    return false;
                }
                let field_tys: Vec<Type> = decl.fields.iter().map(|f| (*f.ty).clone()).collect();
                field_tys
                    .iter()
                    .all(|field_ty| self.type_is_shallow_copy_call_arg_value(field_ty, depth + 1))
            }
            _ => false,
        }
    }
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

    fn struct_decl(name: &str, derives: &[&str], fields: Vec<(&str, Type)>) -> StructDecl {
        StructDecl {
            name: name.to_string(),
            derives: derives.iter().map(|d| d.to_string()).collect(),
            generics: vec![],
            fields: fields
                .into_iter()
                .map(|(n, ty)| Field {
                    name: n.to_string(),
                    ty: Box::new(ty),
                })
                .collect(),
            is_union: false,
            is_opaque: false,
        }
    }

    fn check_with(decls: Vec<StructDecl>) -> MapCheckInfo {
        let mut check = MapCheckInfo::new();
        for decl in decls {
            check.add_struct(decl);
        }
        check
    }

    #[test]
    fn copy_derive_leaves_and_structs() {
        let check = check_with(vec![
            struct_decl("Point", &["copy"], vec![("x", prim(Primitive::I64)), ("y", prim(Primitive::I64))]),
            struct_decl("Owned", &["copy"], vec![("buf", ud("Vec", vec![prim(Primitive::U8)]))]),
            struct_decl("Plain", &[], vec![("x", prim(Primitive::I64))]),
        ]);
        let mut facts = CopyFacts::new(&check);
        assert!(facts.type_has_copy_derive(&prim(Primitive::I32)));
        assert!(!facts.type_has_copy_derive(&prim(Primitive::RawPtr)));
        assert!(facts.type_has_copy_derive(&ud("Point", vec![])));
        // The cache now holds the decision (SAB-D05 mirror point).
        assert_eq!(facts.copy_value_cache.get("Point"), Some(&true));
        // A Copy-derived struct with a non-Copy field is not Copy.
        assert!(!facts.type_has_copy_derive(&ud("Owned", vec![])));
        // No derive at all: not Copy, and cached as such.
        assert!(!facts.type_has_copy_derive(&ud("Plain", vec![])));
        assert_eq!(facts.copy_value_cache.get("Plain"), Some(&false));
        // Unknown struct: false + cached false (Zig's decl-miss arm).
        assert!(!facts.type_has_copy_derive(&ud("Ghost", vec![])));
        assert_eq!(facts.copy_value_cache.get("Ghost"), Some(&false));
        // Std owners never derive, generic or not.
        assert!(!facts.type_has_copy_derive(&ud("Vec", vec![prim(Primitive::I32)])));
    }

    #[test]
    fn copy_value_and_chan_rule() {
        let check = check_with(vec![struct_decl(
            "Point",
            &["copy"],
            vec![("x", prim(Primitive::I64))],
        )]);
        let mut facts = CopyFacts::new(&check);
        assert!(facts.type_is_copy_value(&ud("Chan", vec![prim(Primitive::I32)])));
        assert!(facts.type_is_copy_value(&ud("Point", vec![])));
        assert!(facts.type_is_copy_struct(&ud("Point", vec![])));
        assert!(!facts.type_is_copy_struct(&prim(Primitive::I64)));
        let tuple = Type::Tuple(sla_ast::TupleType {
            elems: vec![prim(Primitive::I32), ud("Point", vec![])],
        });
        assert!(facts.type_is_copy_value(&tuple));
        let array = Type::Array(sla_ast::ArrayType {
            elem: Box::new(ud("String", vec![])),
            len: 2,
        });
        assert!(!facts.type_is_copy_value(&array));
    }

    #[test]
    fn shallow_copy_depth_rules() {
        let check = check_with(vec![struct_decl(
            "Point",
            &[],
            vec![("x", prim(Primitive::I64))],
        )]);
        let mut facts = CopyFacts::new(&check);
        // Primitives are shallow-copy at any depth ≤ 8.
        assert!(facts.type_is_shallow_copy_call_arg_value(&prim(Primitive::I64), 0));
        // Std collections: only when nested (depth > 0).
        let vec_ty = ud("Vec", vec![prim(Primitive::I32)]);
        assert!(!facts.type_is_shallow_copy_call_arg_value(&vec_ty, 0));
        assert!(facts.type_is_shallow_copy_call_arg_value(&vec_ty, 1));
        // Depth cap: beyond 8 the base fact decides false even
        // for a primitive nested in tuples.
        let deep = Type::Tuple(sla_ast::TupleType {
            elems: vec![prim(Primitive::I64)],
        });
        assert!(!facts.type_is_shallow_copy_call_arg_value(&deep, 9));
        // A plain struct of primitives recurses field-wise.
        assert!(facts.type_is_shallow_copy_call_arg_value(&ud("Point", vec![]), 0));
    }
}
