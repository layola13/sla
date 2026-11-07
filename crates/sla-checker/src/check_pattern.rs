//! Enum-pattern typing and binding definition, mirroring
//! `patternBindingType` (type_checker.zig:1591) and
//! `definePatternBindings` (type_checker.zig:1649) of the Zig
//! checker. Shared by the if-let chain, `let else`, `while let`,
//! and the Option/Result form of `match`.
//!
//! The `writable` parameter of the Zig original is passed STRAIGHT
//! into `defineSymbol`'s `is_const` slot — the name says writable,
//! the slot means constant (D-C15 / FB-013). Callers therefore pass
//! `is_const = true` for if-let / while-let / match bindings and
//! `is_const = false` for let-else bindings, exactly as Zig does;
//! the inversion is preserved, not repaired.

use crate::checker::Checker;
use crate::diag::{CheckResult, Diagnostic, ErrorKind};
use crate::types_util::{
    enum_name_matches_decl, option_inner_type, peel_borrow_pointer, result_err_type,
    result_ok_type,
};
use sla_ast::exprs::EnumPattern;
use sla_ast::types::Type;

impl Checker {
    /// The enum declaration name a value type refers to, if it is a
    /// registered enum (Zig `enumDeclForValueType`).
    pub fn enum_decl_name_for_value(&self, value_ty: &Type) -> Option<String> {
        match peel_borrow_pointer(value_ty) {
            Type::UserDefined(u) if self.enum_variants.contains_key(&u.name) => {
                Some(u.name.clone())
            }
            _ => None,
        }
    }

    /// Mirrors Zig `patternBindingType`: for `Option<T>` /
    /// `Result<T, E>` scrutinees, the type of the single pattern
    /// binding, or `None` for the `None` variant. Errors carry the
    /// caller's context string ("if let" / "let else" / "while let"
    /// / "match") exactly as the Zig `comptime context` does.
    pub fn pattern_binding_type(
        &self,
        pattern: &EnumPattern,
        value_ty: &Type,
        context: &str,
    ) -> CheckResult<Option<Type>> {
        if let Some(inner) = option_inner_type(value_ty) {
            if pattern.enum_name != "Option" {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "{context} pattern must match Option<T>, got {}",
                        pattern.enum_name
                    ),
                ));
            }
            match pattern.variant_name.as_str() {
                "Some" => {
                    if pattern.bindings.len() != 1 {
                        return Err(Diagnostic::new(
                            ErrorKind::InvalidArgsCount,
                            "Some pattern requires one binding",
                        ));
                    }
                    return Ok(Some(inner));
                }
                "None" => {
                    if !pattern.bindings.is_empty() {
                        return Err(Diagnostic::new(
                            ErrorKind::InvalidArgsCount,
                            "None pattern requires zero bindings",
                        ));
                    }
                    return Ok(None);
                }
                other => {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!("unsupported Option variant {other} in {context}"),
                    ));
                }
            }
        }
        if let Some(ok_ty) = result_ok_type(value_ty) {
            let err_ty = result_err_type(value_ty).ok_or_else(|| {
                Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    "Result value without error type",
                )
            })?;
            if pattern.enum_name != "Result" {
                return Err(Diagnostic::new(
                    ErrorKind::TypeMismatch,
                    format!(
                        "{context} pattern must match Result<T, E>, got {}",
                        pattern.enum_name
                    ),
                ));
            }
            match pattern.variant_name.as_str() {
                "Ok" => {
                    if pattern.bindings.len() != 1 {
                        return Err(Diagnostic::new(
                            ErrorKind::InvalidArgsCount,
                            "Ok pattern requires one binding",
                        ));
                    }
                    return Ok(Some(ok_ty));
                }
                "Err" => {
                    if pattern.bindings.len() != 1 {
                        return Err(Diagnostic::new(
                            ErrorKind::InvalidArgsCount,
                            "Err pattern requires one binding",
                        ));
                    }
                    return Ok(Some(err_ty));
                }
                other => {
                    return Err(Diagnostic::new(
                        ErrorKind::TypeMismatch,
                        format!("unsupported Result variant {other} in {context}"),
                    ));
                }
            }
        }
        Err(Diagnostic::new(
            ErrorKind::TypeMismatch,
            format!("{context} value must be Option<T> or Result<T, E>"),
        ))
    }

    /// Mirrors Zig `definePatternBindings`: define the pattern's
    /// bindings in the CURRENT frame. `is_const` receives the Zig
    /// `writable` argument verbatim (see the module note, D-C15).
    pub fn define_pattern_bindings(
        &mut self,
        pattern: &EnumPattern,
        value_ty: &Type,
        context: &str,
        is_const: bool,
    ) -> CheckResult<()> {
        if option_inner_type(value_ty).is_some() || result_ok_type(value_ty).is_some() {
            if let Some(ty) = self.pattern_binding_type(pattern, value_ty, context)? {
                // pattern_binding_type has already proven exactly
                // one binding exists on this path.
                self.define_symbol(&pattern.bindings[0], ty, is_const)?;
            }
            return Ok(());
        }
        let decl_name = self.enum_decl_name_for_value(value_ty).ok_or_else(|| {
            Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "{context} value must be Option<T>, Result<T, E>, or enum"
                ),
            )
        })?;
        if !enum_name_matches_decl(&pattern.enum_name, &decl_name) {
            return Err(Diagnostic::new(
                ErrorKind::TypeMismatch,
                format!(
                    "{context} pattern must match enum {decl_name}, got {}",
                    pattern.enum_name
                ),
            ));
        }
        let variants = self
            .enum_variants
            .get(&decl_name)
            .cloned()
            .unwrap_or_default();
        let variant = variants
            .iter()
            .find(|v| v.name == pattern.variant_name)
            .ok_or_else(|| {
                Diagnostic::new(
                    ErrorKind::FieldNotFound,
                    format!(
                        "FieldNotFound: enum {decl_name} has no variant {}",
                        pattern.variant_name
                    ),
                )
            })?;
        if pattern.bindings.len() != variant.fields.len() {
            return Err(Diagnostic::new(
                ErrorKind::InvalidArgsCount,
                format!(
                    "pattern {}::{} expects {} bindings, got {}",
                    pattern.enum_name,
                    pattern.variant_name,
                    variant.fields.len(),
                    pattern.bindings.len()
                ),
            ));
        }
        let fields: Vec<(String, Type)> = variant
            .fields
            .iter()
            .map(|(n, t)| (n.clone(), t.clone()))
            .collect();
        for (binding, (_, field_ty)) in pattern.bindings.iter().zip(fields.iter()) {
            self.define_symbol(binding, field_ty.clone(), is_const)?;
        }
        Ok(())
    }
}
