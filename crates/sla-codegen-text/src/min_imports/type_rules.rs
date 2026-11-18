//! Type-query rules for the minimal-import scan (`Scanner` methods):
//! which type closures mention `String`, resolved through the checker.
//! Part of [`super`] — see the parent module docs for the rule catalogue.

use super::*;

impl<'a> Scanner<'a> {
    /// Resolved `String` mention: the syntactic check, plus user
    /// structs resolved through the checker (a `Foo` whose field
    /// is `String` counts — the clone sites resolve fields the
    /// same way). Depth-capped; unknown structs do not count,
    /// matching `deep_clone_needed` (clone.rs), which returns
    /// false when the declaration is unavailable.
    pub(super) fn ty_mentions_string(&self, ty: &Type, depth: usize) -> bool {
        if ty_has_string(ty) {
            return true;
        }
        if depth > 8 {
            return false;
        }
        match peel_borrow_pointer_type(ty) {
            Type::UserDefined(u) => {
                let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
                    return false;
                };
                decl.fields.iter().any(|f| {
                    let field_ty = substitute_ty(f.ty.as_ref(), &decl.generics, &u.generics);
                    self.ty_mentions_string(&field_ty, depth + 1)
                })
            }
            Type::Array(arr) => self.ty_mentions_string(&arr.elem, depth + 1),
            Type::Tuple(tup) => tup
                .elems
                .iter()
                .any(|e| self.ty_mentions_string(e, depth + 1)),
            _ => false,
        }
    }

    /// Mirror of `deep_clone_needed` (clone.rs) restricted to the
    /// question "can cloning a value of this type emit
    /// STRING_BUF_CLONE": `String` itself, `Vec`/`GrowVec`
    /// elements, `Box` payloads, array/tuple elements, and struct
    /// fields (resolved). `Rc`/`Arc` clone the handle only, maps
    /// and sets are uncloneable, `Option`/`Result` have no clone
    /// arm — all read false here, exactly as in the emitter.
    pub(super) fn clone_mentions_string(&self, ty: &Type, depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        let peeled = peel_borrow_pointer_type(ty);
        if let Type::UserDefined(u) = peeled {
            if u.name == "String" {
                return true;
            }
        }
        if let Some((_, elem)) = crate::place::vec_like_element_type(ty) {
            return self.clone_mentions_string(&elem, depth + 1);
        }
        if let Type::UserDefined(u) = peeled {
            if u.name == "Box" {
                if let Some(inner) = u.generics.first() {
                    return self.clone_mentions_string(inner, depth + 1);
                }
                return false;
            }
            if matches!(u.name.as_str(), "Rc" | "Arc") {
                return false;
            }
            if matches!(
                u.name.as_str(),
                "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet"
            ) {
                return false;
            }
        }
        match peeled {
            Type::Array(arr) => self.clone_mentions_string(&arr.elem, depth + 1),
            Type::Tuple(tup) => tup
                .elems
                .iter()
                .any(|e| self.clone_mentions_string(e, depth + 1)),
            Type::UserDefined(u) => {
                let Some(decl) = crate::place::struct_decl_lookup(self.check, &u.name) else {
                    return false;
                };
                decl.fields.iter().any(|f| {
                    let field_ty = substitute_ty(f.ty.as_ref(), &decl.generics, &u.generics);
                    self.clone_mentions_string(&field_ty, depth + 1)
                })
            }
            _ => false,
        }
    }

    /// The declared type of a struct field, with generics
    /// substituted from the use-site type (`Pair<String>.a`).
    pub(super) fn struct_field_ty(&self, ty: &Type, field_name: &str) -> Option<Type> {
        let peeled = peel_borrow_pointer_type(ty);
        let Type::UserDefined(u) = peeled else {
            return None;
        };
        let decl = crate::place::struct_decl_lookup(self.check, &u.name)?;
        decl.fields.iter().find(|f| f.name == field_name).map(|f| {
            substitute_ty(f.ty.as_ref(), &decl.generics, &u.generics)
        })
    }

    /// First identifier at the root of a field/index projection
    /// (`a.b[0].c` -> `a`; anything else -> `None`). Mirrors
    /// `root_identifier` (sla-lowering): only projections with an
    /// identifier root clone on transfer (call.rs shape C,
    /// into.rs shape D).
    pub(super) fn projection_root(node: &Node) -> Option<String> {
        match node {
            Node::Identifier(name) => Some(name.clone()),
            Node::FieldExpr(f) => Self::projection_root(&f.expr),
            Node::IndexExpr(i) => Self::projection_root(&i.target),
            _ => None,
        }
    }
}
