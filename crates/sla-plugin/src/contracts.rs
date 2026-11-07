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
use sla_checker::{Checker, ExternSig};
use sla_parser::contract::ExternalFunction;
use sla_parser::contract_load::{ContractLoadError, LoadReport};
use std::path::Path;

/// Convert one contract declaration into the checker's registration
/// shape. Only the per-parameter move contract (`^`) and the ABI
/// return spelling survive — that is all `ExternSig` models,
/// mirroring what the Zig checker itself reads out of
/// `ExternalFunction` at the extern call arm (arity + move flags +
/// `setTypeFromAbiReturn`).
pub fn extern_sig_of(func: &ExternalFunction) -> ExternSig {
    ExternSig {
        move_params: func.params.iter().map(|p| p.is_move).collect(),
        ret_abi: func.ret_ty.clone(),
    }
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

/// Zig `loadImportedContracts` + `TypeChecker.loadContracts`,
/// wired: load the contract closure of the program's imports and
/// register every extern into `checker`. The full report (visited
/// files, layout defines, unregistered macro definitions, `.sla`
/// imports) is returned for boundary dumps and accounting.
pub fn load_imported_contracts_into(
    checker: &mut Checker,
    program: &Program,
    source_file: &Path,
) -> Result<LoadReport, ContractLoadError> {
    let paths = decl_import_paths(program);
    let report = sla_parser::contract_load::load_imported_contracts(&paths, source_file)?;
    for func in &report.externs {
        checker.register_extern_func(&func.name, extern_sig_of(func));
    }
    Ok(report)
}

/// The rest of `plugin_import_expand.zig` — `.sla` body inlining,
/// module tables, reachability-driven materialization — is outside
/// the contract slice. Its entry point exists so that any future
/// caller hits a loud refusal, never a silent partial expansion
/// (task rule: unwired places fail loudly).
pub fn expand_sla_imports_unwired() -> ContractLoadError {
    ContractLoadError::UnwiredExpansion(
        "expandSlaImports (plugin_import_expand.zig) is phase-remaining; \
         only the loadImportedContracts slice is wired"
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
        };
        let sig = extern_sig_of(&func);
        assert_eq!(sig.move_params, vec![true]);
        assert_eq!(sig.ret_abi, "i32");
    }

    #[test]
    fn expansion_entry_is_loud_not_silent() {
        let err = expand_sla_imports_unwired();
        assert!(matches!(err, ContractLoadError::UnwiredExpansion(_)));
    }
}
