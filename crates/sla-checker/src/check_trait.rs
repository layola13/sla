//! Trait bookkeeping and the impl-contract check, mirroring
//! `checkImpl` / `checkTraitImpl` and the trait lookup helpers of
//! `type_checker.zig` (:1960-2070, :5900-6010).
//!
//! Contract rules as in Zig: an impl with a trait name must
//! separately impl every supertrait of that trait; it may only
//! contain methods the trait itself declares (inherited methods do
//! NOT legitimize an entry); and every declared method must be
//! present with a signature equal under `Self` substitution.

use crate::checker::{Checker, TraitMethodInfo};
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{concrete_type_name, dyn_trait_name, types_equal};
use sla_ast::decls::ImplDecl;
use sla_ast::node::Node;
use sla_ast::types::Type;

impl Checker {
    /// Mirrors Zig `traitExtendsTrait`: reflexivity plus a
    /// recursive walk over supertraits.
    pub fn trait_extends_trait(&self, trait_name: &str, target: &str) -> bool {
        if trait_name == target {
            return true;
        }
        match self.traits.get(trait_name) {
            Some(decl) => decl
                .supertraits
                .iter()
                .any(|sup| self.trait_extends_trait(sup, target)),
            None => false,
        }
    }

    /// Mirrors Zig `findTraitMethod`: supertraits first, then the
    /// trait's own methods.
    pub fn find_trait_method(
        &self,
        trait_name: &str,
        method_name: &str,
    ) -> Option<TraitMethodInfo> {
        let decl = self.traits.get(trait_name)?;
        for sup in &decl.supertraits {
            if let Some(m) = self.find_trait_method(sup, method_name) {
                return Some(m);
            }
        }
        decl.methods.iter().find(|m| m.name == method_name).cloned()
    }

    /// Mirrors Zig `findTraitOwnMethod`: only the trait's own
    /// declared methods (no inheritance).
    pub fn find_trait_own_method(
        &self,
        trait_name: &str,
        method_name: &str,
    ) -> Option<TraitMethodInfo> {
        self.traits
            .get(trait_name)?
            .methods
            .iter()
            .find(|m| m.name == method_name)
            .cloned()
    }

    /// Mirrors Zig `findTraitDeclaringMethod`: the trait in the
    /// hierarchy that declares the method (supertraits first).
    pub fn find_trait_declaring_method(
        &self,
        trait_name: &str,
        method_name: &str,
    ) -> Option<String> {
        let decl = self.traits.get(trait_name)?;
        for sup in &decl.supertraits {
            if let Some(owner) = self.find_trait_declaring_method(sup, method_name)
            {
                return Some(owner);
            }
        }
        if decl.methods.iter().any(|m| m.name == method_name) {
            return Some(trait_name.to_string());
        }
        None
    }

    /// Mirrors Zig `typeDirectlyImplementsTrait`: an exact
    /// `trait|type` registration, no extension closure.
    pub fn type_directly_implements_trait(
        &self,
        type_name: &str,
        trait_name: &str,
    ) -> bool {
        self.trait_impls.contains(&format!("{trait_name}|{type_name}"))
    }

    /// Mirrors Zig `typeImplementsTraitName`: direct registration,
    /// or any registered impl trait that extends the target.
    pub fn type_implements_trait_name(
        &self,
        type_name: &str,
        trait_name: &str,
    ) -> bool {
        if self.type_directly_implements_trait(type_name, trait_name) {
            return true;
        }
        self.trait_impls.iter().any(|key| {
            let Some((impl_trait, impl_type)) = key.split_once('|') else {
                return false;
            };
            impl_type == type_name && self.trait_extends_trait(impl_trait, trait_name)
        })
    }

    /// Mirrors Zig `typeImplementsTrait`: dyn carriers dispatch by
    /// their trait's extension closure; concrete types by name.
    pub fn type_implements_trait(&self, ty: &Type, trait_name: &str) -> bool {
        if let Some(dyn_name) = dyn_trait_name(ty) {
            return self.trait_extends_trait(dyn_name, trait_name);
        }
        match concrete_type_name(ty) {
            Some(name) => self.type_implements_trait_name(name, trait_name),
            None => false,
        }
    }

    /// Mirrors Zig `typeEqualsWithSelf`: like `typesEqual`, except
    /// an expected user type named exactly `Self` matches the
    /// impl's target type, recursively through carriers.
    pub fn type_equals_with_self(
        &self,
        actual: &Type,
        expected: &Type,
        self_ty: &Type,
    ) -> bool {
        if let Type::UserDefined(u) = expected {
            if u.name == "Self" {
                return types_equal(actual, self_ty);
            }
        }
        match (actual, expected) {
            (Type::Borrow(a), Type::Borrow(e)) => {
                self.type_equals_with_self(a, e, self_ty)
            }
            (Type::Pointer(a), Type::Pointer(e)) => {
                self.type_equals_with_self(a, e, self_ty)
            }
            (Type::Array(a), Type::Array(e)) => {
                a.len == e.len && self.type_equals_with_self(&a.elem, &e.elem, self_ty)
            }
            (Type::Tuple(a), Type::Tuple(e)) => {
                a.elems.len() == e.elems.len()
                    && a.elems
                        .iter()
                        .zip(e.elems.iter())
                        .all(|(x, y)| self.type_equals_with_self(x, y, self_ty))
            }
            (Type::Future(a), Type::Future(e)) => {
                self.type_equals_with_self(a, e, self_ty)
            }
            (Type::FnPtr(a), Type::FnPtr(e)) => {
                a.params.len() == e.params.len()
                    && a.params
                        .iter()
                        .zip(e.params.iter())
                        .all(|(x, y)| self.type_equals_with_self(x, y, self_ty))
                    && self.type_equals_with_self(&a.ret, &e.ret, self_ty)
            }
            (Type::UserDefined(a), Type::UserDefined(e)) => {
                a.name == e.name
                    && a.generics.len() == e.generics.len()
                    && a.generics
                        .iter()
                        .zip(e.generics.iter())
                        .all(|(x, y)| self.type_equals_with_self(x, y, self_ty))
            }
            _ => types_equal(actual, expected),
        }
    }

    /// Mirrors Zig `checkImpl`: the trait contract (when the impl
    /// names a trait) plus a normal function check of every method.
    /// Each method is checked with `current_self_ty` set to the impl
    /// target so the parser-preserved `Self` type resolves exactly
    /// where Zig resolves it (inside the method check).
    pub fn check_impl(&mut self, imp: &ImplDecl) -> CheckResult<()> {
        if let Some(trait_name) = &imp.trait_name {
            self.check_trait_impl(trait_name, imp)?;
        }
        for method in &imp.methods {
            let Node::FuncDecl(f) = method else {
                return Err(Diagnostic::new(
                    ErrorKind::CompileError,
                    "impl block contains a non-function member",
                ));
            };
            if !f.is_decl_only && !f.is_extern {
                let saved = self
                    .current_self_ty
                    .replace((*imp.target_ty).clone());
                let r = self.check_func(f);
                self.current_self_ty = saved;
                r?;
            }
        }
        Ok(())
    }

    /// Mirrors Zig `checkTraitImpl` (type_checker.zig:~5970).
    fn check_trait_impl(
        &mut self,
        trait_name: &str,
        imp: &ImplDecl,
    ) -> CheckResult<()> {
        let trait_decl = self.traits.get(trait_name).cloned().ok_or_else(|| {
            Diagnostic::new(
                ErrorKind::UndefinedVariable,
                format!("UndefinedVariable: trait `{trait_name}`"),
            )
        })?;
        let target_name = crate::check_program::target_type_name(&imp.target_ty)?;
        for sup in &trait_decl.supertraits {
            if !self.type_directly_implements_trait(&target_name, sup) {
                return Err(Diagnostic::new(
                    ErrorKind::UndefinedVariable,
                    format!(
                        "Trait impl `{trait_name}` for `{target_name}` requires separate impl of supertrait `{sup}`"
                    ),
                ));
            }
        }
        for method in &imp.methods {
            let Node::FuncDecl(f) = method else {
                return Err(Diagnostic::new(
                    ErrorKind::CompileError,
                    "impl block contains a non-function member",
                ));
            };
            if self.find_trait_own_method(trait_name, &f.name).is_none() {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "Trait impl for `{target_name}` contains method `{}` not declared by trait `{trait_name}`",
                        f.name
                    ),
                ));
            }
        }
        for trait_method in &trait_decl.methods {
            let impl_method = imp.methods.iter().find_map(|m| match m {
                Node::FuncDecl(f) if f.name == trait_method.name => Some(f),
                _ => None,
            });
            let impl_method = impl_method.ok_or_else(|| {
                Diagnostic::new(
                    ErrorKind::UndefinedVariable,
                    format!(
                        "Trait impl missing method `{}` for trait `{trait_name}`",
                        trait_method.name
                    ),
                )
            })?;
            if impl_method.params.len() != trait_method.params.len() {
                return Err(Diagnostic::new(
                    ErrorKind::InvalidArgsCount,
                    format!(
                        "trait method `{}` arity mismatch",
                        trait_method.name
                    ),
                ));
            }
            for (actual, expected) in
                impl_method.params.iter().zip(trait_method.params.iter())
            {
                if actual.is_borrow != expected.is_borrow
                    || actual.is_move != expected.is_move
                {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "trait method `{}` parameter capability mismatch",
                            trait_method.name
                        ),
                    ));
                }
                if !self.type_equals_with_self(
                    &actual.ty,
                    &expected.ty,
                    &imp.target_ty,
                ) {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!(
                            "trait method `{}` parameter type mismatch",
                            trait_method.name
                        ),
                    ));
                }
            }
            if !self.type_equals_with_self(
                &impl_method.ret_ty,
                &trait_method.ret_ty,
                &imp.target_ty,
            ) {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "trait method `{}` return type mismatch",
                        trait_method.name
                    ),
                ));
            }
        }
        Ok(())
    }
}
