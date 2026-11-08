//! Checker alias resolution split out of checker.rs for the
//! 700-line file cap (body unchanged).

use crate::checker::Checker;
use sla_ast::types::Type;

impl Checker {
    /// Resolve a user-defined name through the alias table.
    pub fn resolve_alias<'a>(&'a self, ty: &'a Type) -> &'a Type {
        if let Type::UserDefined(u) = ty {
            if u.generics.is_empty() {
                if let Some(target) = self.aliases.get(&u.name) {
                    return target;
                }
            }
        }
        ty
    }
}
