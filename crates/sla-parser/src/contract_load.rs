//! Imported-contract loading: the `loadImportedContracts` slice of
//! `sa_plugin_sla/src/plugin_import_expand.zig` (the recursive
//! loader at :1338-1447 plus `scanExpandedSourceImports` at :91),
//! with path resolution from `imports` (plugin_imports.zig) and
//! contract parsing from `contract` (contract_parser.zig).
//!
//! Mirrored order of operations, per resolved file:
//!   1. mark visited (by canonical path, first-visit wins);
//!   2. expand the source (`source_expand.expandForModulePath` —
//!      only the no-directive fast path is ported; a source that
//!      uses `@expand_tuple` fails loudly, see INT2-7);
//!   3. scan the expanded source for nested `@import`s and load
//!      them recursively (nested contracts land BEFORE the file's
//!      own, as in Zig);
//!   4. `.sai` -> extern declarations, `.sal` -> layout defines.
//!
//! What this slice deliberately does NOT do (phase-remaining, and
//! reported loudly in the `LoadReport` instead of silently skipped):
//!   * `.sla` inline expansion of imported bodies;
//!   * imported `[MACRO]` registration (`plugin_imported_macros.zig`)
//!     — macro names encountered are listed in the report;
//!   * layout defines have no table in the Rust checker — they are
//!     carried in the report, not dropped (INT2-3).

use crate::contract::{parse_sai, parse_sal, ContractParseError, ExternalFunction, LayoutDefine};
use crate::imports::{
    expanded_source_may_contain_imports, import_path_from_line, resolve_import_files, ImportError,
};
use std::path::{Path, PathBuf};

/// The outcome of a contract load. All lists are in deterministic
/// load order (program declaration order, then per-file scan order);
/// nothing here is built by iterating a hash map.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// Extern declarations in registration order. Duplicate names
    /// are kept: the caller registers in this order and a later
    /// registration overwrites an earlier one, exactly like Zig's
    /// `extern_funcs.put`.
    pub externs: Vec<ExternalFunction>,
    /// Layout defines in load order (no checker table exists for
    /// them yet — INT2-3).
    pub layout_defines: Vec<LayoutDefine>,
    /// Canonical paths of every file visited, in first-visit order.
    pub visited: Vec<String>,
    /// `[MACRO]` definitions encountered but NOT registered
    /// (phase-remaining): (file, macro names in source order).
    pub macro_defs: Vec<(String, Vec<String>)>,
    /// Resolved `.sla` imports whose bodies were NOT expanded
    /// (phase-remaining), in visit order.
    pub sla_imports: Vec<String>,
}

impl LoadReport {
    /// Extern names in registration order (with duplicates).
    pub fn extern_names(&self) -> Vec<&str> {
        self.externs.iter().map(|f| f.name.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractLoadError {
    Import(ImportError),
    Parse {
        file: String,
        kind: ContractParseError,
    },
    /// A contract-closure source uses `@expand_tuple`; the source
    /// expansion port is phase-remaining (INT2-7). Loud by design:
    /// Zig expands these silently, we refuse rather than scan an
    /// unexpanded source and register a wrong contract set.
    UnsupportedExpandTuple(String),
    /// The full import-expansion entry (`.sla` body inlining) is
    /// not wired in this slice.
    UnwiredExpansion(String),
}

impl std::fmt::Display for ContractLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractLoadError::Import(e) => write!(f, "import resolution failed: {e}"),
            ContractLoadError::Parse { file, kind } => {
                write!(f, "contract parse failed in {file}: {kind}")
            }
            ContractLoadError::UnsupportedExpandTuple(file) => {
                write!(f, "@expand_tuple expansion not ported (in {file})")
            }
            ContractLoadError::UnwiredExpansion(what) => {
                write!(f, "import expansion not wired in this slice: {what}")
            }
        }
    }
}

impl std::error::Error for ContractLoadError {}

impl From<ImportError> for ContractLoadError {
    fn from(e: ImportError) -> Self {
        ContractLoadError::Import(e)
    }
}

struct LoadState {
    visited: Vec<String>,
    report: LoadReport,
}

/// Zig `loadImportedContracts` (plugin_import_expand.zig:1429):
/// walk the program's `@import` declarations in order and load the
/// contract closure of each. `source_file` is the importing file;
/// it is excluded from its own closure (Zig `source_abs`).
pub fn load_imported_contracts(
    decl_import_paths: &[String],
    source_file: &Path,
) -> Result<LoadReport, ContractLoadError> {
    let source_dir = source_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let source_abs = crate::imports::real_path(source_file);
    let mut state = LoadState {
        visited: Vec::new(),
        report: LoadReport::default(),
    };
    for import_path in decl_import_paths {
        load_recursive(&source_dir, import_path, Some(&source_abs), &mut state)?;
    }
    state.report.visited = state.visited;
    Ok(state.report)
}

/// Zig `loadImportContractsRecursive` (:1338) +
/// `scanExpandedSourceImports` (:91), fused: resolve, visit, scan
/// nested imports first, then load this file's own contracts.
fn load_recursive(
    base_dir: &Path,
    import_path: &str,
    exclude_path: Option<&str>,
    state: &mut LoadState,
) -> Result<(), ContractLoadError> {
    let resolved_list = resolve_import_files(base_dir, import_path, exclude_path)?;
    for resolved in &resolved_list {
        if state.visited.iter().any(|p| p == &resolved.path) {
            continue;
        }
        state.visited.push(resolved.path.clone());

        let import_dir = Path::new(&resolved.path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| base_dir.to_path_buf());

        // source_expand.expandForModulePath fast path: without the
        // directive the expansion is the identity.
        if resolved.source.contains("@expand_tuple") {
            return Err(ContractLoadError::UnsupportedExpandTuple(
                resolved.path.clone(),
            ));
        }
        let expanded: &str = &resolved.source;

        // Nested imports first (Zig scans before loading contracts).
        if expanded_source_may_contain_imports(expanded) {
            for line in expanded.split('\n') {
                if let Some(child_import) = import_path_from_line(line) {
                    load_recursive(
                        &import_dir,
                        &child_import,
                        Some(&resolved.path),
                        state,
                    )?;
                }
            }
        }

        let macro_names = scan_macro_names(expanded);
        if !macro_names.is_empty() {
            state
                .report
                .macro_defs
                .push((resolved.path.clone(), macro_names));
        }

        if resolved.path.ends_with(".sla") {
            state.report.sla_imports.push(resolved.path.clone());
        } else if resolved.path.ends_with(".sai") {
            let funcs = parse_sai(expanded).map_err(|kind| ContractLoadError::Parse {
                file: resolved.path.clone(),
                kind,
            })?;
            state.report.externs.extend(funcs);
        } else if resolved.path.ends_with(".sal") {
            let defs = parse_sal(expanded).map_err(|kind| ContractLoadError::Parse {
                file: resolved.path.clone(),
                kind,
            })?;
            state.report.layout_defines.extend(defs);
        }
    }
    Ok(())
}

/// `[MACRO] NAME ...` headers in an SA-text source, in source order.
/// Reporting only: imported-macro registration is phase-remaining.
fn scan_macro_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("[MACRO]") {
            if let Some(name) = rest.split_whitespace().next() {
                names.push(name.to_string());
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new(tag: &str) -> TempTree {
            let dir = std::env::temp_dir().join(format!(
                "sla_contract_load_{}_{}",
                tag,
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempTree(dir)
        }
        fn write(&self, rel: &str, content: &str) {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn loads_direct_and_nested_contracts_in_zig_order() {
        let tree = TempTree::new("nested");
        tree.write(
            "sub/inner.sai",
            "@extern inner_fn(x: i32) -> i32\n",
        );
        tree.write(
            "sub/outer.sai",
            "@import \"inner.sai\"\n@extern outer_fn() -> void\n",
        );
        tree.write("sub/layout.sal", "#def Thing_SIZE = 16\n");
        tree.write(
            "main.sla",
            "@import \"sub/outer.sai\"\n@import \"sub/layout.sal\"\nfn main() -> i32 { return 0; }\n",
        );
        let report = load_imported_contracts(
            &["sub/outer.sai".to_string(), "sub/layout.sal".to_string()],
            &tree.0.join("main.sla"),
        )
        .unwrap();
        // Nested (inner) registers before the importing file (outer),
        // matching Zig's scan-before-load order.
        assert_eq!(report.extern_names(), vec!["inner_fn", "outer_fn"]);
        assert_eq!(report.layout_defines.len(), 1);
        assert_eq!(report.layout_defines[0].name, "Thing_SIZE");
        assert_eq!(report.layout_defines[0].val_int, 16);
        assert_eq!(report.visited.len(), 3);
    }

    #[test]
    fn missing_import_is_a_loud_file_not_found() {
        let tree = TempTree::new("missing");
        tree.write("main.sla", "fn main() -> i32 { return 0; }\n");
        let err = load_imported_contracts(
            &["nope.sai".to_string()],
            &tree.0.join("main.sla"),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ContractLoadError::Import(ImportError::FileNotFound(_))
        ));
    }

    #[test]
    fn sa_file_macros_are_reported_not_registered() {
        let tree = TempTree::new("macros");
        tree.write(
            "sub/m.sa",
            "[MACRO] FOO %out\n    call @x()\n[END_MACRO]\n",
        );
        tree.write("main.sla", "@import \"sub/m.sa\"\n");
        let report =
            load_imported_contracts(&["sub/m.sa".to_string()], &tree.0.join("main.sla"))
                .unwrap();
        assert_eq!(report.macro_defs.len(), 1);
        assert_eq!(report.macro_defs[0].1, vec!["FOO".to_string()]);
        assert!(report.externs.is_empty());
    }

    #[test]
    fn expand_tuple_in_closure_is_loud() {
        let tree = TempTree::new("expand");
        tree.write(
            "sub/e.sai",
            "@expand_tuple(1, 2, T)\n@extern f() -> void\n",
        );
        tree.write("main.sla", "@import \"sub/e.sai\"\n");
        let err =
            load_imported_contracts(&["sub/e.sai".to_string()], &tree.0.join("main.sla"))
                .unwrap_err();
        assert!(matches!(
            err,
            ContractLoadError::UnsupportedExpandTuple(_)
        ));
    }
}
