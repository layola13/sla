//! Plugin wiring layer for imported contracts — coordination
//! ruling D-A2's home for the `loadImportedContracts` slice of
//! `plugin_import_expand.zig`.
//!
//! Division of labor (D-A2): `.sai`/`.sal` parsing lives in
//! `sla_parser::contract`, path resolution in `sla_parser::imports`,
//! the recursive load orchestration in
//! `sla_parser::contract_load` (shared with sla-oracle, which must
//! consume the same machinery — it cannot depend on this crate
//! under the frozen Cargo graph; see docs/phase3/int2.md). What
//! lives HERE, as in Zig's plugin layer, is the checker-facing
//! registration: converting each contract declaration into an
//! `ExternSig` and filling the checker's registry through its one
//! existing entry point, `Checker::register_extern_func`.
//!
//! Registration-surface limits, hit and registered rather than
//! worked around (docs/divergences/contracts.md):
//!   * `ExternSig` has no `return_fallible` field — the flag is
//!     carried in the `LoadReport` but dropped at registration
//!     (INT2-2; the Zig checker itself only stores it for codegen);
//!   * `ImportedMacroInfo` has no `assigned_output_mask` field —
//!     the mask is carried in the record but dropped at
//!     registration (IMP-2, same shape as INT2-2);
//!   * layout defines have no checker table to land in (INT2-3);
//!   * `println` / `str_eq` are NOT contracts in Zig — they are
//!     checker name-specials (type_checker.zig isPrintlnCall /
//!     isStrEqCall arms). They cannot be expressed through
//!     `register_extern_func` (fixed arity, untyped bypass), and
//!     faking them there would invert Zig verdicts (venue probes
//!     docs/probes/int2/). The missing arms are a checker-lane gap,
//!     registered as INT2-1; this layer registers only what the
//!     contracts actually declare.

use sla_ast::node::Node;
use sla_ast::Program;
use sla_checker::{Checker, ExternSig, ImportedMacroInfo};
use sla_parser::imported_macros::ImportedMacroRecord;
use sla_parser::contract::ExternalFunction;
use sla_parser::contract_load::{ContractLoadError, LoadReport};
use std::path::Path;

/// Convert one contract declaration into the checker's registration
/// shape, carrying the parameter spellings the R-6 overload
/// unification needs (candidate identity and matching both work on
/// spellings, exactly as Zig's contract registration does).
pub fn extern_sig_of(func: &ExternalFunction) -> ExternSig {
    sla_checker::check_overload3::extern_sig_from_parts(
        &func
            .params
            .iter()
            .map(|pp| (pp.name.clone(), pp.ty.clone(), pp.is_borrow, pp.is_move))
            .collect::<Vec<_>>(),
        &func.ret_ty,
        func.return_fallible,
        func.is_variadic,
    )
}

/// The program's `@import` paths, in declaration order (Zig
/// `loadImportedContracts` walks `program.program.decls` the same
/// way).
pub fn decl_import_paths(program: &Program) -> Vec<String> {
    program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Node::ImportDecl(import) => Some(import.path.clone()),
            _ => None,
        })
        .collect()
}

/// Convert one imported-macro record into the checker's
/// registration shape. `assigned_output_mask` has no field to land
/// in (IMP-2) and is dropped here, audibly, in the same way
/// `return_fallible` is dropped for externs (INT2-2).
pub fn imported_macro_info_of(record: &ImportedMacroRecord) -> ImportedMacroInfo {
    ImportedMacroInfo {
        arity: record.arity,
        leading_outputs: record.leading_outputs,
        import_path: record.import_path.clone(),
        borrowed_arg_mask: record.borrowed_arg_mask,
        address_slot_arg_mask: record.address_slot_arg_mask,
        direct_callees: record.direct_callees.clone(),
    }
}

/// Zig `loadImportedContracts` + `TypeChecker.loadContracts`,
/// wired: load the contract closure of the program's imports and
/// register every extern AND every imported macro into `checker`
/// (macros since phase 4 — INT2-4 closed; the records are generated
/// from each imported file's own declarations and merged in
/// deterministic load order with per-file provenance retained in
/// the report, the federation-shaped registration direction of the
/// 2026-11-07 ruling — see docs/divergences/import-expand.md
/// IMP-7). The full report (visited files, layout defines, `.sla`
/// imports) is returned for boundary dumps and accounting.
pub fn load_imported_contracts_into(
    checker: &mut Checker,
    program: &Program,
    source_file: &Path,
) -> Result<LoadReport, ContractLoadError> {
    let paths = decl_import_paths(program);
    let report = sla_parser::contract_load::load_imported_contracts(&paths, source_file)?;
    for func in &report.externs {
        // R-6 follow-up: register under the contract discipline
        // (identical duplicates dedupe; distinct same-name
        // declarations promote into extern overload sets).
        let _ = checker.register_extern_contract(&func.name, extern_sig_of(func));
    }
    for record in &report.imported_macro_registry {
        checker.register_imported_macro(&record.name, imported_macro_info_of(record));
    }
    Ok(report)
}

/// The `.sla` body-inlining entry of `plugin_import_expand.zig`.
/// Phase 4 wires the include-all combination in
/// `sla_plugin::sla_expand::expand_sla_imports`; the DEFAULT
/// selective combination remains unwired, and this entry point
/// exists so that a caller asking for the default (no options)
/// hits a loud refusal, never a silent partial expansion (task
/// rule: unwired places fail loudly).
pub fn expand_sla_imports_unwired() -> ContractLoadError {
    ContractLoadError::UnwiredExpansion(
        "expandSlaImports with default (selective/reachability) options \
         is not wired; the include-all combination lives in \
         sla_plugin::sla_expand::expand_sla_imports"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extern_sig_conversion_keeps_move_flags_and_ret() {
        let func = ExternalFunction {
            name: "sa_fmt_buffer_free".to_string(),
            params: vec![sla_parser::contract::ContractParam {
                name: "buffer".to_string(),
                ty: "u64".to_string(),
                is_borrow: false,
                is_move: true,
            }],
            ret_ty: "i32".to_string(),
            return_fallible: false,
            is_variadic: false,
        };
        let sig = extern_sig_of(&func);
        assert_eq!(sig.move_params, vec![true]);
        assert_eq!(sig.ret_abi, "i32");
    }

    #[test]
    fn imported_macro_info_conversion_drops_only_assigned_mask() {
        let record = ImportedMacroRecord {
            name: "FOO".to_string(),
            arity: 2,
            leading_outputs: 1,
            import_path: Some("m.sa".to_string()),
            borrowed_arg_mask: 0b10,
            address_slot_arg_mask: 0b01,
            assigned_output_mask: 0b01,
            direct_callees: vec!["helper".to_string()],
        };
        let info = imported_macro_info_of(&record);
        assert_eq!(info.arity, 2);
        assert_eq!(info.leading_outputs, 1);
        assert_eq!(info.borrowed_arg_mask, 0b10);
        assert_eq!(info.address_slot_arg_mask, 0b01);
        assert_eq!(info.direct_callees, vec!["helper".to_string()]);
    }

    #[test]
    fn expansion_entry_is_loud_not_silent() {
        let err = expand_sla_imports_unwired();
        assert!(matches!(err, ContractLoadError::UnwiredExpansion(_)));
    }
}
