//! Trait vtable declarations and dyn-method slot resolution,
//! mirrored from codegen.zig (`emitTraitVTableDecl` :1052,
//! `emitTraitVTableEntries`) and lowering_rules.zig
//! (`vtableName` :6098, `traitMethodCount` :6106,
//! `dynMethodSlot` :6116).
//!
//! A trait impl emits one `@const VT_{Type}_{Trait} = vtable {
//! ... }` whose slots are the trait's methods — supertrait
//! methods first, recursively — each pointing at the lowered
//! symbol of the impl method under its trait-mangled name. A
//! slot whose impl method is registered decl-only is skipped:
//! no function body is emitted for it, so a slot pointing at
//! its symbol would reference an unknown register at the host.

use sla_ast::{FuncDecl, ImplDecl};
use sla_lowering::TraitDeclView;

use crate::codegen::Codegen;
use crate::{unsupported, Result};

impl<'c> Codegen<'c> {
    /// lowering_rules.zig:6098 `vtableName`.
    pub(crate) fn trait_vtable_name(trait_name: &str, type_name: &str) -> String {
        format!("VT_{type_name}_{trait_name}")
    }

    /// The trait's declaration shape: the checker's trait table
    /// through CheckInfo when the adapter supplies it, else the
    /// program's own TraitDecl nodes (the `trait_index` collected
    /// in `generate_program`).
    pub(crate) fn trait_decl_view(&self, name: &str) -> Option<TraitDeclView> {
        if let Some(view) = self.check.trait_decl(name) {
            return Some(view);
        }
        let (_, decl) = self.trait_index.iter().find(|(n, _)| n == name)?;
        Some(TraitDeclView {
            supertraits: decl.supertraits.clone(),
            methods: decl
                .methods
                .iter()
                .map(|m| sla_lowering::TraitMethodView {
                    name: m.name.clone(),
                    params: m.params.clone(),
                    ret_ty: (*m.ret_ty).clone(),
                })
                .collect(),
        })
    }

    /// codegen.zig:1052 `emitTraitVTableDecl`.
    pub(crate) fn emit_trait_vtable_decl(&mut self, decl: &ImplDecl) -> Result<()> {
        let trait_name = match &decl.trait_name {
            Some(t) => t.clone(),
            None => return Ok(()),
        };
        let type_name = match sla_lowering::concrete_type_name(&decl.target_ty) {
            Some(n) => n.to_string(),
            None => {
                return unsupported(
                    "trait impl vtable on non-user-defined type",
                    "codegen.zig:1052 (emitTraitVTableDecl)",
                )
            }
        };
        let vt_name = Self::trait_vtable_name(&trait_name, &type_name);
        let mut entries: Vec<String> = Vec::new();
        self.collect_trait_vtable_entries(&trait_name, &type_name, &mut entries)?;
        self.wl(&format!(
            "@const {vt_name} = vtable {{ {} }}",
            entries.join(", ")
        ));
        Ok(())
    }

    /// codegen.zig `emitTraitVTableEntries`: supertraits first
    /// (recursive), then the trait's own methods in declaration
    /// order. A method whose impl is registered decl-only (the
    /// assoc index carries every impl method's FuncDecl) is
    /// skipped, matching the set of emitted function bodies.
    fn collect_trait_vtable_entries(
        &self,
        trait_name: &str,
        type_name: &str,
        entries: &mut Vec<String>,
    ) -> Result<()> {
        let view = match self.trait_decl_view(trait_name) {
            Some(v) => v,
            None => {
                return Err(crate::TextError::Unsupported {
                    feature: format!(
                        "trait impl vtable: trait `{trait_name}` has no declaration"
                    ),
                    zig_ref: "codegen.zig:1052 (emitTraitVTableDecl)",
                })
            }
        };
        for supertrait in &view.supertraits {
            self.collect_trait_vtable_entries(supertrait, type_name, entries)?;
        }
        for method in &view.methods {
            let mangled = sla_lowering::mangle_trait_method_name(
                type_name,
                trait_name,
                &method.name,
            );
            if let Some((_, f)) = self
                .assoc_index
                .iter()
                .find(|(n, _)| n == &mangled)
            {
                if f.is_decl_only {
                    continue;
                }
            }
            let lowered = self.lowered_func_symbol(&mangled);
            entries.push(format!("{} = @{lowered}", method.name));
        }
        Ok(())
    }

    /// The checker's `findTraitDeclaringMethod`: the trait in
    /// the hierarchy rooted at `trait_name` that declares the
    /// method (the trait itself first, then supertraits).
    pub(crate) fn find_trait_declaring_method(
        &self,
        trait_name: &str,
        method_name: &str,
    ) -> Option<String> {
        let view = self.trait_decl_view(trait_name)?;
        if view.methods.iter().any(|m| m.name == method_name) {
            return Some(trait_name.to_string());
        }
        for supertrait in &view.supertraits {
            if let Some(d) = self.find_trait_declaring_method(supertrait, method_name) {
                return Some(d);
            }
        }
        None
    }

    /// The checker's `resolveTraitMethodSymbol` (check_method.rs)
    /// projected onto the codegen's assoc index: the unique
    /// trait-impl method symbol for a receiver type, optionally
    /// pinned to one trait (the `Trait::method(recv, ...)` form).
    /// The declaring trait names the symbol, the impl registers
    /// the body under it, and the declared parameter count must
    /// equal the call's argument count. Ambiguity or absence
    /// yields None (the checker has already rejected ambiguous
    /// programs, so a miss here stays on the caller's loud path).
    pub(crate) fn resolve_trait_method_decl(
        &self,
        type_name: &str,
        target_trait: Option<&str>,
        method_name: &str,
        arg_count: usize,
    ) -> Option<(String, FuncDecl)> {
        if let Some(trait_name) = target_trait {
            let declaring = self.find_trait_declaring_method(trait_name, method_name)?;
            let symbol =
                sla_lowering::mangle_trait_method_name(type_name, &declaring, method_name);
            let (_, decl) = self
                .assoc_index
                .iter()
                .find(|(n, _)| n == &symbol)?;
            if decl.params.len() != arg_count {
                return None;
            }
            return Some((symbol, decl.clone()));
        }
        let prefix = format!("{type_name}__");
        let suffix = format!("_{method_name}");
        let mut found: Vec<(String, FuncDecl)> = Vec::new();
        for (key, decl) in &self.assoc_index {
            let Some(rest) = key.strip_prefix(&prefix) else {
                continue;
            };
            let Some(trait_name) = rest.strip_suffix(&suffix) else {
                continue;
            };
            if trait_name.is_empty() || decl.params.len() != arg_count {
                continue;
            }
            // The key's trait segment must actually declare the
            // method (its own declaration, not an inherited slot
            // the vtable re-exports under the supertrait's name).
            let view = match self.trait_decl_view(trait_name) {
                Some(v) => v,
                None => continue,
            };
            if !view.methods.iter().any(|m| m.name == method_name) {
                continue;
            }
            if !found.iter().any(|(k, _)| k == key) {
                found.push((key.clone(), decl.clone()));
            }
        }
        if found.len() == 1 {
            return found.pop();
        }
        None
    }

    /// lowering_rules.zig:6106 `traitMethodCount`: the trait's
    /// total method count, supertraits included (recursive).
    pub(crate) fn trait_method_count(&self, trait_name: &str) -> Option<usize> {
        let view = self.trait_decl_view(trait_name)?;
        let mut count = 0usize;
        for supertrait in &view.supertraits {
            count += self.trait_method_count(supertrait)?;
        }
        count += view.methods.len();
        Some(count)
    }

    /// lowering_rules.zig:6116 `dynMethodSlot`: the byte offset
    /// of a method's vtable slot — supertrait methods occupy the
    /// leading slots, each slot is 8 bytes.
    pub(crate) fn dyn_method_slot(
        &self,
        trait_name: &str,
        method_name: &str,
    ) -> Option<usize> {
        let view = self.trait_decl_view(trait_name)?;
        let mut base = 0usize;
        for supertrait in &view.supertraits {
            if let Some(slot) = self.dyn_method_slot(supertrait, method_name) {
                return Some(base + slot);
            }
            base += self.trait_method_count(supertrait)? * 8;
        }
        for (i, method) in view.methods.iter().enumerate() {
            if method.name == method_name {
                return Some(base + i * 8);
            }
        }
        None
    }
}
