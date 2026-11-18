//! The monomorphizer — a whole-program specialization pass that
//! mirrors `sa_plugin_sla/src/monomorphizer.zig` (FB-012 port).
//!
//! Pipeline position (plugin_compile.zig `runSlaFrontend`): after
//! import expansion and the test-filter prune, before contract
//! load and the type check. The pass
//!
//!   1. rejects a name that carries a generic function template
//!      together with any other function declaration
//!      (`GenericOverloadConflict` — generic templates do not
//!      participate in overloading, so the name must stand alone);
//!   2. separates generic struct / enum / function / impl
//!      templates out of the program (templates never reach the
//!      checker, D-C11);
//!   3. rewrites every use site in the remaining declarations:
//!      generic types are replaced by mangled concrete instances,
//!      `GenericFuncRef` values become plain identifiers naming a
//!      materialized instance, explicit-generic calls and
//!      shallowly-inferable calls become calls of the mangled
//!      instance (see `node::specialize_node`);
//!   4. appends the materialized instances as ordinary
//!      declarations with `generics` cleared.
//!
//! Mangled names follow the Zig rule exactly (`base` + `_` +
//! type-name per type argument, `types::append_type_name`), so
//! the text and SAB backends see the same symbols the Zig
//! pipeline produces.
//!
//! The submodules split the two recursive walks the way the Zig
//! original is organized: `types` (type specialization /
//! substitution / mangling / call-argument inference), `node`
//! (the specializing node walk), `subst` (the substituting node
//! walk used to clone template bodies).

mod node;
mod subst;
mod types;

use crate::decls::{EnumDecl, FuncDecl, ImplDecl, StructDecl};
use crate::node::Node;
use crate::types::Type;
use std::collections::{HashMap, HashSet};

/// Mirrors Zig `MonomorphizeError`. The Zig side channels carry
/// the offending name on the side (`missingTemplateName` /
/// `genericOverloadConflictName`); here it travels in the
/// variant, and the pipeline renders the same message text the
/// Zig command layer prints (plugin_compile.zig).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonomorphizeError {
    /// A generic function reference (or generic call) named a
    /// function that was never registered as a generic template.
    TemplateNotFound(String),
    /// A generic function template shares its name with another
    /// function declaration.
    GenericOverloadConflict(String),
    /// Any other specialization failure (the Zig catch-all).
    MonomorphizeError,
}

impl std::fmt::Display for MonomorphizeError {
    /// The diagnostic text mirrors `plugin_compile.zig`'s
    /// rendering of the Zig errors byte-for-byte.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MonomorphizeError::GenericOverloadConflict(name) => write!(
                f,
                "Monomorphization Error: generic function `{name}` shares its name with another function declaration; generic templates do not participate in overloading, so a name with a generic template must stand alone (error.GenericOverloadConflict)"
            ),
            MonomorphizeError::TemplateNotFound(name) => write!(
                f,
                "Monomorphization Error: failed to specialize generics: error.TemplateNotFound: {name}"
            ),
            MonomorphizeError::MonomorphizeError => write!(
                f,
                "Monomorphization Error: failed to specialize generics: error.MonomorphizeError"
            ),
        }
    }
}

impl std::error::Error for MonomorphizeError {}

/// Entry point mirroring `Monomorphizer.monomorphize`: transforms
/// the program in place. A non-program root is left untouched
/// (the pipeline only ever passes a program).
pub fn monomorphize_program(root: &mut Node) -> Result<(), MonomorphizeError> {
    if let Node::Program(program) = root {
        let mut mono = Monomorphizer::new();
        mono.monomorphize(program)?;
    }
    Ok(())
}

/// Mirrors Zig `Monomorphizer`. Templates are stored by value
/// (the Rust AST is owned); instantiation clones the template,
/// substitutes the type arguments into it, and then runs the
/// specializing walk over the clone — the same two-step the Zig
/// `instantiateFunction` performs (`substituteBlock` then
/// `specializeBlock`).
pub struct Monomorphizer {
    pub(crate) struct_templates: HashMap<String, StructDecl>,
    pub(crate) enum_templates: HashMap<String, EnumDecl>,
    pub(crate) func_templates: HashMap<String, FuncDecl>,
    pub(crate) impl_templates: Vec<ImplDecl>,
    pub(crate) specialized_structs: HashSet<String>,
    pub(crate) specialized_enums: HashSet<String>,
    pub(crate) specialized_impls: HashSet<String>,
    pub(crate) specialized_funcs: HashSet<String>,
    pub(crate) new_decls: Vec<Node>,
}

impl Monomorphizer {
    pub fn new() -> Self {
        Monomorphizer {
            struct_templates: HashMap::new(),
            enum_templates: HashMap::new(),
            func_templates: HashMap::new(),
            impl_templates: Vec::new(),
            specialized_structs: HashSet::new(),
            specialized_enums: HashSet::new(),
            specialized_impls: HashSet::new(),
            specialized_funcs: HashSet::new(),
            new_decls: Vec::new(),
        }
    }

    pub fn monomorphize(
        &mut self,
        program: &mut crate::decls::Program,
    ) -> Result<(), MonomorphizeError> {
        // Function overloading boundary (the Zig pre-check):
        // generic templates are indexed by name alone and
        // instantiated before type checking, so a name cannot
        // carry a generic template together with any other
        // function declaration.
        {
            let mut template_names: HashSet<&str> = HashSet::new();
            let mut concrete_names: HashSet<&str> = HashSet::new();
            for decl in &program.decls {
                let Node::FuncDecl(f) = decl else { continue };
                if !f.generics.is_empty() {
                    if template_names.contains(f.name.as_str())
                        || concrete_names.contains(f.name.as_str())
                    {
                        return Err(MonomorphizeError::GenericOverloadConflict(f.name.clone()));
                    }
                    template_names.insert(f.name.as_str());
                } else {
                    if template_names.contains(f.name.as_str()) {
                        return Err(MonomorphizeError::GenericOverloadConflict(f.name.clone()));
                    }
                    concrete_names.insert(f.name.as_str());
                }
            }
        }

        // 1. Separate templates from concrete declarations.
        let mut regular_decls: Vec<Node> = Vec::new();
        for decl in std::mem::take(&mut program.decls) {
            match decl {
                Node::StructDecl(s) if !s.generics.is_empty() => {
                    self.struct_templates.insert(s.name.clone(), s);
                }
                Node::EnumDecl(e) if !e.generics.is_empty() => {
                    self.enum_templates.insert(e.name.clone(), e);
                }
                Node::FuncDecl(f) if !f.generics.is_empty() => {
                    self.func_templates.insert(f.name.clone(), f);
                }
                Node::ImplDecl(i) if impl_has_generic_target(&i) => {
                    self.impl_templates.push(i);
                }
                other => regular_decls.push(other),
            }
        }

        // 2. Scan all regular declarations for generic
        // instantiations (types, function references, calls).
        // Instances materialize into `new_decls` as a side effect
        // and are NOT re-scanned here — instantiation already runs
        // the specializing walk over the cloned template body.
        let mut specialized: Vec<Node> = Vec::with_capacity(regular_decls.len());
        for decl in regular_decls {
            specialized.push(self.specialize_node(decl)?);
        }

        // 3. Append all newly generated specialized declarations.
        specialized.append(&mut self.new_decls);
        program.decls = specialized;
        Ok(())
    }

    pub(crate) fn template_not_found(&self, name: &str) -> MonomorphizeError {
        MonomorphizeError::TemplateNotFound(name.to_string())
    }

    /// Mirrors Zig `instantiateFunction`: records the instance
    /// first (the recursion guard for self-referential generics),
    /// substitutes the type arguments into params / return /
    /// body, then specializes the result so nested generic uses
    /// inside the body materialize their own instances.
    pub(crate) fn instantiate_function(
        &mut self,
        mangled_name: &str,
        template: &FuncDecl,
        args: &[Type],
    ) -> Result<(), MonomorphizeError> {
        self.specialized_funcs.insert(mangled_name.to_string());

        let mut spec_params = Vec::with_capacity(template.params.len());
        for p in &template.params {
            let spec_ty = self.substitute_type(&p.ty, &template.generics, args);
            spec_params.push(crate::decls::Param {
                name: p.name.clone(),
                ty: Box::new(self.specialize_type(spec_ty)?),
                is_borrow: p.is_borrow,
                is_move: p.is_move,
                fallible_sugar: p.fallible_sugar,
                is_var: p.is_var,
            });
        }
        let spec_ret = self.substitute_type(&template.ret_ty, &template.generics, args);
        let spec_ret = self.specialize_type(spec_ret)?;
        let spec_body = self.substitute_block(template.body.clone(), &template.generics, args);
        let spec_body = self.specialize_block(spec_body)?;

        self.new_decls.push(Node::FuncDecl(FuncDecl {
            name: mangled_name.to_string(),
            is_pub: template.is_pub,
            is_extern: template.is_extern,
            abi: template.abi.clone(),
            no_mangle: template.no_mangle,
            is_decl_only: template.is_decl_only,
            generics: Vec::new(),
            params: spec_params,
            ret_ty: Box::new(spec_ret),
            body: spec_body,
            is_inline: template.is_inline,
            is_async: template.is_async,
            operator: template.operator,
            is_arena: template.is_arena,
            ret_fallible_sugar: template.ret_fallible_sugar,
            ret_borrow_origin: template.ret_borrow_origin.clone(),
            is_variadic: template.is_variadic,
        }));
        Ok(())
    }

    /// Mirrors Zig `instantiateStruct`.
    pub(crate) fn instantiate_struct(
        &mut self,
        mangled_name: &str,
        template: &StructDecl,
        args: &[Type],
    ) -> Result<(), MonomorphizeError> {
        self.specialized_structs.insert(mangled_name.to_string());

        let mut spec_fields = Vec::with_capacity(template.fields.len());
        for f in &template.fields {
            let spec_ty = self.substitute_type(&f.ty, &template.generics, args);
            spec_fields.push(crate::decls::Field {
                name: f.name.clone(),
                ty: Box::new(self.specialize_type(spec_ty)?),
            });
        }
        self.new_decls.push(Node::StructDecl(StructDecl {
            name: mangled_name.to_string(),
            derives: template.derives.clone(),
            generics: Vec::new(),
            fields: spec_fields,
            is_union: template.is_union,
            is_opaque: template.is_opaque,
        }));
        self.instantiate_generic_impls(&template.name.clone(), mangled_name, args)
    }

    /// Mirrors Zig `instantiateEnum`.
    pub(crate) fn instantiate_enum(
        &mut self,
        mangled_name: &str,
        template: &EnumDecl,
        args: &[Type],
    ) -> Result<(), MonomorphizeError> {
        self.specialized_enums.insert(mangled_name.to_string());

        let mut spec_variants = Vec::with_capacity(template.variants.len());
        for variant in &template.variants {
            let mut spec_fields = Vec::with_capacity(variant.fields.len());
            for f in &variant.fields {
                let spec_ty = self.substitute_type(&f.ty, &template.generics, args);
                spec_fields.push(crate::decls::Field {
                    name: f.name.clone(),
                    ty: Box::new(self.specialize_type(spec_ty)?),
                });
            }
            spec_variants.push(crate::decls::EnumVariant {
                name: variant.name.clone(),
                fields: spec_fields,
                discriminant: variant.discriminant,
            });
        }
        self.new_decls.push(Node::EnumDecl(EnumDecl {
            name: mangled_name.to_string(),
            generics: Vec::new(),
            variants: spec_variants,
        }));
        self.instantiate_generic_impls(&template.name.clone(), mangled_name, args)
    }

    /// Mirrors Zig `instantiateGenericImpls`: every impl template
    /// whose target base name and generic arity match gets a
    /// concrete impl against the mangled target type, once per
    /// (mangled target, trait, method-name set) key.
    fn instantiate_generic_impls(
        &mut self,
        base_name: &str,
        mangled_name: &str,
        args: &[Type],
    ) -> Result<(), MonomorphizeError> {
        for template in self.impl_templates.clone() {
            let Type::UserDefined(target_ud) = &*template.target_ty else {
                continue;
            };
            if target_ud.name != base_name || target_ud.generics.len() != args.len() {
                continue;
            }
            let mut key = String::from(mangled_name);
            key.push_str("|impl|");
            key.push_str(template.trait_name.as_deref().unwrap_or("inherent"));
            for method in &template.methods {
                let Node::FuncDecl(m) = method else {
                    return Err(MonomorphizeError::MonomorphizeError);
                };
                key.push('|');
                key.push_str(&m.name);
            }
            if self.specialized_impls.contains(&key) {
                continue;
            }
            self.specialized_impls.insert(key);

            let generic_names = generic_names_from_target(&template.target_ty)?;
            let substituted = self.substitute_block(template.methods.clone(), &generic_names, args);
            let specialized_methods = self.specialize_block(substituted)?;
            self.new_decls.push(Node::ImplDecl(ImplDecl {
                trait_name: template.trait_name.clone(),
                target_ty: Box::new(Type::UserDefined(crate::types::UserDefined {
                    name: mangled_name.to_string(),
                    generics: Vec::new(),
                })),
                methods: specialized_methods,
            }));
        }
        Ok(())
    }
}

/// Mirrors Zig `implHasGenericTarget`.
fn impl_has_generic_target(decl: &ImplDecl) -> bool {
    matches!(&*decl.target_ty, Type::UserDefined(ud) if !ud.generics.is_empty())
}

/// Mirrors Zig `genericNamesFromTarget`: the impl target's type
/// arguments must themselves be plain user-defined names (the
/// template's generic parameter names).
fn generic_names_from_target(target_ty: &Type) -> Result<Vec<String>, MonomorphizeError> {
    let Type::UserDefined(ud) = target_ty else {
        return Err(MonomorphizeError::MonomorphizeError);
    };
    let mut names = Vec::with_capacity(ud.generics.len());
    for generic_ty in &ud.generics {
        let Type::UserDefined(generic_ud) = generic_ty else {
            return Err(MonomorphizeError::MonomorphizeError);
        };
        if !generic_ud.generics.is_empty() {
            return Err(MonomorphizeError::MonomorphizeError);
        }
        names.push(generic_ud.name.clone());
    }
    Ok(names)
}

impl Default for Monomorphizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
