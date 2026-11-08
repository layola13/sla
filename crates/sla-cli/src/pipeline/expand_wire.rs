//! IMP-4 wiring: imported-function signature registration —
//! the checker-facing half of Zig's
//! `registerImportedFunctionAliasesFromResolvedImports`
//! (plugin_import_expand.zig) that the Rust front-end can
//! consume. The expansion layer only carries the data back
//! (`ExpandedProgram::root_sla_modules`); registration happens
//! here, in the pipeline's registration sequence (after
//! contract loading, before `check_program` — Zig's order).
//!
//! For every root-level `.sla` module, in root declaration
//! order:
//!   * each top-level function registers under its bare name
//!     AND under its `namespace__name` alias (namespace = the
//!     module file stem, Zig `moduleNamespaceFromImportPath`
//!     over the resolved output path);
//!   * each impl method registers under its mangled symbol
//!     only (`{Type}_{method}`, or `{Type}__{Trait}_{method}`
//!     for trait impls) — no alias, exactly as in Zig.
//! Overload-declaration methods are NOT part of this surface
//! in Zig and are not registered here either (1:1 mirror, no
//! improvement). The alias METADATA table
//! (`registerFunctionAliasWithMetadata`, feeding the Zig
//! checker's function-alias tables) is not ported: no Rust
//! consumer reads it. That remainder is the IMP-4 residual in
//! docs/divergences/import-expand.md.
//!
//! Registration is per-name insert (later wins), Zig's `put`
//! semantics, and iterates ordered Vecs only — no hash-map
//! iteration order reaches any behavior.

use sla_ast::node::Node;
use sla_ast::FuncDecl;
use sla_checker::{Checker, FuncSig, ParamSig};

/// Zig `FunctionSignature` as the checker receives it: params,
/// return type, is_async. The Rust record's fallible-sugar and
/// borrow-origin provenance flags have no counterpart in Zig's
/// imported signatures and stay unset; `is_pub` / `is_variadic`
/// travel from the declaration itself.
fn func_sig(func: &FuncDecl) -> FuncSig {
    FuncSig {
        params: func.params.iter().map(ParamSig::from).collect(),
        ret_ty: (*func.ret_ty).clone(),
        is_pub: func.is_pub,
        ret_fallible_sugar: false,
        ret_borrow_origin: None,
        is_async: func.is_async,
        is_variadic: func.is_variadic,
    }
}

pub fn register_imported_signatures(
    checker: &mut Checker,
    root_sla_modules: &[sla_plugin::sla_expand::RootSlaModule],
) {
    for module in root_sla_modules {
        for decl in &module.decls {
            match decl {
                Node::FuncDecl(func) => {
                    let sig = func_sig(func);
                    checker.register_imported_signature(&func.name, sig.clone());
                    let alias = format!("{}__{}", module.namespace, func.name);
                    checker.register_imported_signature(&alias, sig);
                }
                Node::ImplDecl(impl_decl) => {
                    let Some(type_name) =
                        sla_checker::types_util::concrete_type_name(&impl_decl.target_ty)
                    else {
                        continue;
                    };
                    for method in &impl_decl.methods {
                        if let Node::FuncDecl(func) = method {
                            let symbol = match &impl_decl.trait_name {
                                Some(trait_name) => {
                                    sla_lowering::names::mangle_trait_method_name(
                                        &type_name,
                                        trait_name,
                                        &func.name,
                                    )
                                }
                                None => sla_lowering::names::mangle_method_name(
                                    &type_name,
                                    &func.name,
                                ),
                            };
                            checker.register_imported_signature(&symbol, func_sig(func));
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
