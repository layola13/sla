//! `.sla` function-body inline expansion — the
//! `expandSlaImportsWithModuleTable` machinery of
//! `sa_plugin_sla/src/plugin_import_expand.zig` (box head 2cc12ea8),
//! wired for ONE options combination, loudly refusing the rest.
//!
//! WIRED: `include_all_imported_decls = true` — the combination the
//! real `sla check` command runs (plugin_commands.zig at the box
//! head: module table with `parse_function_bodies = false`, then
//! expansion with `imported_bodies_decl_only = true,
//! include_all_imported_decls = true`). In this mode Zig skips
//! reachability entirely, collects the transitive `.sla` module
//! closure children-first, and appends module declarations through
//! `appendModuleDeclsSelective` with an empty reachable set. This
//! module mirrors that path statement-for-statement, including its
//! derived consequences:
//!   * imported `@test` declarations are dropped;
//!   * with `imported_bodies_decl_only`, imported functions and
//!     impl/overload methods are converted to decl-only stubs
//!     (venue probe: an imported function whose body would fail
//!     type checking does NOT fail `sa sla check` — its body never
//!     reaches the checker);
//!   * non-`.sla` imports OF IMPORTED MODULES are retained only via
//!     `shouldRetainResolvedContractImport` against the reachable /
//!     referenced sets — both empty in this mode, and a module
//!     "needs contracts" only through reachable bodies — so they
//!     are NOT retained (venue probe nest_check: a nested `.sai`
//!     extern is never registered on the check path, and the check
//!     still passes because the calling body is decl-only);
//!   * non-`.sla` imports of the ROOT program are appended
//!     unconditionally as import declarations and collected into
//!     `contract_imports` when they need contract loading.
//!
//! ALSO WIRED (phase-6 census R-2): the include-all combination
//! with `imported_bodies_decl_only = false` — module functions
//! keep their bodies, and modules' non-`.sla` imports are
//! retained (appended as declarations and collected into
//! `contract_imports`), because the kept bodies call those
//! contracts. This is the combination the CLI's TestCodegen
//! front-end runs so `sla test` bodies can call imported module
//! functions. The decl-only combination above is byte-identical
//! to before.
//!
//! LOUDLY UNWIRED (returns `SlaExpandError::SelectiveUnwired`):
//! the default selective mode (plugin_reachability.zig, 2,762
//! lines at the box head: reachable-symbol computation, lazy
//! transitive discovery, body materialization/reparse, reachable
//! plan caches) and every combination that depends on it
//! (`prune_for_test_codegen`, `lazy_transitive_sla_imports`,
//! `load_reachable_imported_bodies_from_registry`). The IMP-4
//! registration itself lives in the CLI wiring (sla-cli
//! `pipeline::expand_wire`); this layer only carries the root
//! groups' modules back in `ExpandedProgram::root_sla_modules`.
//!
//! Module parsing approximation (IMP-5): Zig's parser prescans a
//! module's `.sla` imports from inside the parser
//! (`prescanSlaImportTypes`, D-P11). The Rust parser takes an
//! explicit seed instead; this layer seeds from a textual scan of
//! the transitive `.sla` import sources' `struct`/`enum` lines.
//! Decl-only mode (the check path) barely depends on the seed;
//! raw-body mode may diverge on imported-type struct literals
//! whose declarations the textual scan misses.

use sla_ast::node::Node;
use sla_ast::{FuncDecl, Program};
use sla_parser::contract_load::ContractLoadError;
use sla_parser::imports::{
    import_path_from_line, module_namespace_from_import_path, resolve_import_files, ImportError,
    ResolvedImport,
};
use sla_parser::{Parser, ParserOptions};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Zig `SlaImportExpansionOptions` (plugin_module_table.zig),
/// defaults identical.
#[derive(Debug, Clone, Default)]
pub struct SlaImportExpansionOptions {
    pub prune_for_test_codegen: bool,
    pub test_filter: Option<String>,
    pub imported_bodies_decl_only: bool,
    pub load_reachable_imported_bodies_from_registry: bool,
    pub lazy_transitive_sla_imports: bool,
    pub include_all_imported_decls: bool,
}

/// One root-level `.sla` import as parsed by the module table
/// (IMP-4 surface): output-path file stem + own decls.
#[derive(Debug)]
pub struct RootSlaModule {
    pub namespace: String,
    pub path: String,
    pub decls: Vec<Node>,
}

/// The expansion result. `primary` is parallel to `program.decls`
/// and plays the role of Zig's `primary_decls` pointer set (the
/// Rust pipeline keys side tables by node address — D-T10 — so the
/// caller must keep these exact nodes; the flags travel alongside
/// instead of a second set type).
#[derive(Debug)]
pub struct ExpandedProgram {
    pub program: Program,
    pub primary: Vec<bool>,
    /// Zig `contract_imports`: resolved non-`.sla` imports that
    /// need contract loading, in append order.
    pub contract_imports: Vec<ResolvedImport>,
    /// Module canonical paths in collection order (children before
    /// parents), for boundary dumps.
    pub ordered_modules: Vec<String>,
    /// Root import groups' `.sla` modules (IMP-4 data), in root
    /// declaration order; the CLI registers them (expand_wire).
    pub root_sla_modules: Vec<RootSlaModule>,
    /// Bodies-mode only: modules whose full-body parse failed
    /// (an unported parser surface inside a module body or
    /// signature — impl blocks, `&[T][owner]`, `T!`, `go`) and
    /// which were therefore included decl-only. Recorded, never
    /// silent: calls into such a module resolve by signature at
    /// the checker and fail loudly downstream (no body exists to
    /// emit), while modules the program never calls stop
    /// blocking it. Empty in decl-only mode.
    pub body_fallback_modules: Vec<String>,
}

#[derive(Debug)]
pub enum SlaExpandError {
    Contract(ContractLoadError),
    Parse { file: String, kind: sla_parser::ParseError },
    InvalidExpandTuple(String),
    /// The requested options combination needs the reachability
    /// machinery, which is not ported (see module docs).
    SelectiveUnwired(String),
}

impl std::fmt::Display for SlaExpandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlaExpandError::Contract(e) => write!(f, "{e}"),
            SlaExpandError::Parse { file, kind } => {
                write!(f, "parse failed in {file}: {kind:?}")
            }
            SlaExpandError::InvalidExpandTuple(file) => {
                write!(f, "error.InvalidExpandTuple (in {file})")
            }
            SlaExpandError::SelectiveUnwired(what) => {
                write!(f, "import expansion not wired for this options combination: {what}")
            }
        }
    }
}

impl std::error::Error for SlaExpandError {}

impl From<ImportError> for SlaExpandError {
    fn from(e: ImportError) -> Self {
        SlaExpandError::Contract(ContractLoadError::Import(e))
    }
}

// -- module table (plugin_module_table.zig, include-all subset) -----

struct SlaModule {
    path: String,
    #[allow(dead_code)]
    output_path: String,
    program: Program,
    resolved_imports: Vec<ResolvedImport>,
}

struct SlaModuleTable {
    modules: Vec<SlaModule>,
    decl_only_parse: bool,
    body_fallback_modules: Vec<String>,
}

impl SlaModuleTable {
    /// Zig `SlaModuleTable.getOrParse`: expand, parse, build the
    /// resolved-import list. Type-name seeding is the IMP-5 textual
    /// approximation described in the module docs.
    fn get_or_parse(&mut self, resolved: &ResolvedImport) -> Result<usize, SlaExpandError> {
        if let Some(idx) = self.modules.iter().position(|m| m.path == resolved.path) {
            return Ok(idx);
        }
        let expanded =
            sla_parser::source_expand::expand_for_module_path(&resolved.path, &resolved.source)
                .map_err(|_| SlaExpandError::InvalidExpandTuple(resolved.path.clone()))?;
        let base_dir = Path::new(&resolved.path)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let decl_only_options = ParserOptions {
            parse_function_bodies: false,
            function_body_names: None,
            parse_macro_bodies: true,
            macro_body_names: None,
            parse_test_bodies: false,
        };
        let options = if self.decl_only_parse {
            decl_only_options.clone()
        } else {
            ParserOptions::default()
        };
        let parse_with = |options: ParserOptions| -> Result<Node, sla_parser::ParseError> {
            let mut parser = Parser::with_options(&expanded, options);
            let (types, enums) = prescan_import_type_names(&base_dir, &expanded);
            if !types.is_empty() || !enums.is_empty() {
                let type_refs: Vec<&str> = types.iter().map(String::as_str).collect();
                let enum_refs: Vec<&str> = enums.iter().map(String::as_str).collect();
                parser.seed_known_type_names(&type_refs, &enum_refs);
            }
            parser.parse_program()
        };
        let node = match parse_with(options) {
            Ok(node) => node,
            Err(kind) => {
                // Bodies mode only: a module whose bodies (or a
                // signature inside it) use a parser surface the
                // Rust parser has not ported — impl blocks,
                // `&[T][owner]`, `T!`, `go` — must not take down
                // programs that import it transitively. Retry
                // decl-only for THIS module and record it in
                // body_fallback_modules (see ExpandedProgram).
                // The decl-only parse is the same one the check
                // path has always used; if it also fails, the
                // original error stands.
                if self.decl_only_parse {
                    return Err(SlaExpandError::Parse {
                        file: resolved.path.clone(),
                        kind,
                    });
                }
                match parse_with(decl_only_options) {
                    Ok(node) => {
                        self.body_fallback_modules.push(resolved.path.clone());
                        node
                    }
                    Err(_) => {
                        return Err(SlaExpandError::Parse {
                            file: resolved.path.clone(),
                            kind,
                        })
                    }
                }
            }
        };
        let Node::Program(program) = node else {
            return Err(SlaExpandError::Parse {
                file: resolved.path.clone(),
                kind: sla_parser::ParseError::new(
                    sla_parser::ParseErrorKind::UnexpectedToken,
                    0,
                    0,
                ),
            });
        };
        let mut resolved_imports = Vec::new();
        for decl in &program.decls {
            if let Node::ImportDecl(import) = decl {
                let mut files =
                    resolve_import_files(&base_dir, &import.path, Some(&resolved.path))?;
                resolved_imports.append(&mut files);
            }
        }
        self.modules.push(SlaModule {
            path: resolved.path.clone(),
            output_path: resolved.output_path.clone(),
            program,
            resolved_imports,
        });
        Ok(self.modules.len() - 1)
    }
}

/// Textual prescan of the transitive `.sla` imports' declared
/// struct/enum names (IMP-5 approximation of the parser prescan).
fn prescan_import_type_names(base_dir: &Path, expanded_source: &str) -> (Vec<String>, Vec<String>) {
    let mut types = Vec::new();
    let mut enums = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: Vec<(PathBuf, String)> = vec![(base_dir.to_path_buf(), expanded_source.to_string())];
    while let Some((dir, source)) = queue.pop() {
        for line in source.split('\n') {
            if let Some(child) = import_path_from_line(line) {
                if !child.ends_with(".sla") {
                    continue;
                }
                if let Ok(files) = resolve_import_files(&dir, &child, None) {
                    for file in files {
                        if !visited.insert(file.path.clone()) {
                            continue;
                        }
                        collect_declared_type_names(&file.source, &mut types, &mut enums);
                        let child_dir = Path::new(&file.path)
                            .parent()
                            .map(Path::to_path_buf)
                            .unwrap_or_else(|| dir.clone());
                        queue.push((child_dir, file.source.clone()));
                    }
                }
            }
        }
    }
    (types, enums)
}

fn collect_declared_type_names(source: &str, types: &mut Vec<String>, enums: &mut Vec<String>) {
    for line in source.lines() {
        let line = line.trim_start();
        let line = line.strip_prefix("pub ").unwrap_or(line);
        for (kw, out) in [("struct ", &mut *types), ("enum ", &mut *enums)] {
            if let Some(rest) = line.strip_prefix(kw) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() && !out.contains(&name) {
                    out.push(name);
                }
            }
        }
    }
}

// -- selective-append pieces (include-all mode) ----------------------

/// Zig `resolvedImportNeedsContractLoading`.
fn resolved_import_needs_contract_loading(resolved: &ResolvedImport) -> bool {
    if resolved.path.ends_with(".sai") || resolved.path.ends_with(".sal") {
        return true;
    }
    if !resolved.path.ends_with(".sa") {
        return false;
    }
    resolved.source.contains("[MACRO]")
        || resolved.source.contains("@import")
        || resolved.source.contains("@expand_tuple")
}

/// Zig `appendResolvedNonSlaImportDecl`.
fn append_resolved_non_sla_import_decl(
    resolved: &ResolvedImport,
    out_decls: &mut Vec<Node>,
    primary: &mut Vec<bool>,
    contract_imports: &mut Vec<ResolvedImport>,
) {
    out_decls.push(Node::ImportDecl(sla_ast::ImportDecl {
        path: resolved.output_path.clone(),
    }));
    primary.push(true);
    if resolved_import_needs_contract_loading(resolved) {
        contract_imports.push(resolved.clone());
    }
}

/// Zig `makeDeclOnlyFuncNode` / `maybeDeclOnlyFuncNode` on owned
/// nodes: force converts a non-decl-only function into a stub with
/// an empty body.
fn decl_only_func(func: &FuncDecl) -> FuncDecl {
    let mut stub = func.clone();
    stub.is_decl_only = true;
    stub.body = Vec::new();
    stub
}

fn maybe_decl_only_func_node(node: &Node, force_decl_only: bool) -> Node {
    match node {
        Node::FuncDecl(func) if force_decl_only && !func.is_decl_only => {
            Node::FuncDecl(decl_only_func(func))
        }
        _ => node.clone(),
    }
}

/// Zig `appendFilteredImplDeclWithOptions`, include-all arm: every
/// method is kept (converted per decl-only), non-function methods
/// pass through; the original decl is reused when nothing changed.
fn append_filtered_impl_decl_include_all(
    decl: &Node,
    options: &SlaImportExpansionOptions,
    out_decls: &mut Vec<Node>,
) {
    let Node::ImplDecl(impl_decl) = decl else {
        out_decls.push(decl.clone());
        return;
    };
    if sla_checker::types_util::concrete_type_name(&impl_decl.target_ty).is_none() {
        out_decls.push(decl.clone());
        return;
    }
    let mut methods = Vec::new();
    let mut changed = false;
    for method in &impl_decl.methods {
        if let Node::FuncDecl(func) = method {
            if options.imported_bodies_decl_only && !func.is_decl_only {
                changed = true;
            }
            methods.push(maybe_decl_only_func_node(
                method,
                options.imported_bodies_decl_only,
            ));
        } else {
            methods.push(method.clone());
        }
    }
    if !changed && methods.len() == impl_decl.methods.len() {
        out_decls.push(decl.clone());
    } else if !methods.is_empty() {
        let mut pruned = impl_decl.clone();
        pruned.methods = methods;
        out_decls.push(Node::ImplDecl(pruned));
    }
}

/// Zig `appendFilteredOverloadDeclWithOptions`, include-all arm.
fn append_filtered_overload_decl_include_all(
    decl: &Node,
    options: &SlaImportExpansionOptions,
    out_decls: &mut Vec<Node>,
) {
    let Node::OverloadDecl(overload_decl) = decl else {
        out_decls.push(decl.clone());
        return;
    };
    if sla_checker::types_util::concrete_type_name(&overload_decl.target_ty).is_none() {
        out_decls.push(decl.clone());
        return;
    }
    let mut methods = Vec::new();
    for method in &overload_decl.methods {
        if matches!(method, Node::FuncDecl(_)) {
            methods.push(maybe_decl_only_func_node(
                method,
                options.imported_bodies_decl_only,
            ));
        } else {
            methods.push(method.clone());
        }
    }
    if methods.len() == overload_decl.methods.len() && !options.imported_bodies_decl_only {
        out_decls.push(decl.clone());
    } else if !methods.is_empty() {
        let mut pruned = overload_decl.clone();
        pruned.methods = methods;
        out_decls.push(Node::OverloadDecl(pruned));
    }
}

/// Zig `collectSlaModulesRecursive`: children first, then the
/// module itself; visited by canonical path.
fn collect_sla_modules_recursive(
    table: &mut SlaModuleTable,
    module_idx: usize,
    visited: &mut Vec<String>,
    ordered: &mut Vec<usize>,
) -> Result<(), SlaExpandError> {
    let path = table.modules[module_idx].path.clone();
    if visited.iter().any(|p| p == &path) {
        return Ok(());
    }
    visited.push(path);
    let children: Vec<ResolvedImport> = table.modules[module_idx]
        .resolved_imports
        .iter()
        .filter(|r| r.path.ends_with(".sla"))
        .cloned()
        .collect();
    for child in &children {
        let child_idx = table.get_or_parse(child)?;
        collect_sla_modules_recursive(table, child_idx, visited, ordered)?;
    }
    ordered.push(module_idx);
    Ok(())
}

/// Zig `appendModuleDeclsSelective`, include-all arm (reachable
/// and referenced sets are empty; see module docs for the derived
/// retention outcome).
fn append_module_decls_include_all(
    table: &mut SlaModuleTable,
    module_idx: usize,
    emitted: &mut Vec<String>,
    options: &SlaImportExpansionOptions,
    out_decls: &mut Vec<Node>,
    primary: &mut Vec<bool>,
    contract_imports: &mut Vec<ResolvedImport>,
) -> Result<(), SlaExpandError> {
    let path = table.modules[module_idx].path.clone();
    if emitted.iter().any(|p| p == &path) {
        return Ok(());
    }
    emitted.push(path);

    // Children before the module's own declarations (non-lazy path).
    let children: Vec<ResolvedImport> = table.modules[module_idx]
        .resolved_imports
        .iter()
        .filter(|r| r.path.ends_with(".sla"))
        .cloned()
        .collect();
    for child in &children {
        let child_idx = table.get_or_parse(child)?;
        append_module_decls_include_all(
            table,
            child_idx,
            emitted,
            options,
            out_decls,
            primary,
            contract_imports,
        )?;
    }

    let decls: Vec<Node> = table.modules[module_idx].program.decls.clone();
    let resolved_imports: Vec<ResolvedImport> =
        table.modules[module_idx].resolved_imports.clone();
    for decl in &decls {
        match decl {
            Node::ImportDecl(_) => {
                for child_resolved in &resolved_imports {
                    if child_resolved.path.ends_with(".sla") {
                        continue;
                    }
                    // Decl-only mode: shouldRetainResolvedContractImport
                    // against empty reachable/referenced sets is always
                    // false (module docs) — nothing is retained.
                    // Bodies mode (imported_bodies_decl_only = false):
                    // the kept bodies CALL these contracts, so the
                    // import is retained exactly like a root import —
                    // appended as a declaration and collected into
                    // contract_imports when it needs contract loading.
                    if !options.imported_bodies_decl_only {
                        append_resolved_non_sla_import_decl(
                            child_resolved,
                            out_decls,
                            primary,
                            contract_imports,
                        );
                    }
                }
            }
            Node::FuncDecl(_) => {
                let node = maybe_decl_only_func_node(
                    decl,
                    options.imported_bodies_decl_only,
                );
                out_decls.push(node);
                primary.push(true);
            }
            Node::ImplDecl(_) => {
                let before = out_decls.len();
                append_filtered_impl_decl_include_all(decl, options, out_decls);
                if out_decls.len() != before {
                    primary.push(true);
                }
            }
            Node::OverloadDecl(_) => {
                let before = out_decls.len();
                append_filtered_overload_decl_include_all(decl, options, out_decls);
                if out_decls.len() != before {
                    primary.push(true);
                }
            }
            Node::TestDecl(_) => {
                // Zig: test declarations of imported modules are
                // not appended.
            }
            _ => {
                // Macro (include-all keeps it), const (include-all
                // keeps it), and every other declaration kind are
                // appended raw.
                out_decls.push(decl.clone());
                primary.push(true);
            }
        }
    }
    Ok(())
}

/// Zig `expandSlaImportsWithModuleTableUsingContractTypeChecker`
/// for the wired combination; every other combination is a loud
/// `SelectiveUnwired` (module docs).
pub fn expand_sla_imports(
    program: &Program,
    source_file: &Path,
    options: &SlaImportExpansionOptions,
) -> Result<ExpandedProgram, SlaExpandError> {
    if !options.include_all_imported_decls {
        return Err(SlaExpandError::SelectiveUnwired(
            "selective expansion (plugin_reachability.zig reachable-symbol \
             computation + body materialization) is not ported"
                .to_string(),
        ));
    }
    if options.prune_for_test_codegen {
        return Err(SlaExpandError::SelectiveUnwired(
            "prune_for_test_codegen filters root declarations by the \
             reachable set, which is not ported"
                .to_string(),
        ));
    }
    if options.lazy_transitive_sla_imports {
        return Err(SlaExpandError::SelectiveUnwired(
            "lazy_transitive_sla_imports discovery is reachability-driven \
             and not ported"
                .to_string(),
        ));
    }
    if options.load_reachable_imported_bodies_from_registry {
        return Err(SlaExpandError::SelectiveUnwired(
            "load_reachable_imported_bodies_from_registry keeps reachable \
             bodies via the materialization machinery, which is not ported"
                .to_string(),
        ));
    }

    let source_dir = source_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let source_abs = sla_parser::imports::real_path(source_file);
    let mut table = SlaModuleTable {
        modules: Vec::new(),
        decl_only_parse: options.imported_bodies_decl_only,
        body_fallback_modules: Vec::new(),
    };

    // Root import groups + module collection (children first).
    let mut root_groups: Vec<Vec<ResolvedImport>> = Vec::new();
    let mut root_sla_modules: Vec<RootSlaModule> = Vec::new();
    let mut visited: Vec<String> = Vec::new();
    let mut ordered: Vec<usize> = Vec::new();
    for decl in &program.decls {
        if let Node::ImportDecl(import) = decl {
            let resolved = resolve_import_files(&source_dir, &import.path, Some(&source_abs))?;
            for r in &resolved {
                if r.path.ends_with(".sla") {
                    let idx = table.get_or_parse(r)?;
                    root_sla_modules.push(RootSlaModule {
                        namespace: module_namespace_from_import_path(&r.output_path),
                        path: r.path.clone(),
                        decls: table.modules[idx].program.decls.clone(),
                    });
                    collect_sla_modules_recursive(&mut table, idx, &mut visited, &mut ordered)?;
                }
            }
            root_groups.push(resolved);
        } else {
            root_groups.push(Vec::new());
        }
    }
    let ordered_modules: Vec<String> = ordered
        .iter()
        .map(|&idx| table.modules[idx].path.clone())
        .collect();

    // Flatten: root declarations in order, imports replaced.
    let mut out_decls: Vec<Node> = Vec::new();
    let mut primary: Vec<bool> = Vec::new();
    let mut contract_imports: Vec<ResolvedImport> = Vec::new();
    let mut emitted: Vec<String> = Vec::new();
    for (decl, group) in program.decls.iter().zip(root_groups.iter()) {
        if let Node::ImportDecl(_) = decl {
            for resolved in group {
                if resolved.path.ends_with(".sla") {
                    let idx = table.get_or_parse(resolved)?;
                    append_module_decls_include_all(
                        &mut table,
                        idx,
                        &mut emitted,
                        options,
                        &mut out_decls,
                        &mut primary,
                        &mut contract_imports,
                    )?;
                } else {
                    append_resolved_non_sla_import_decl(
                        resolved,
                        &mut out_decls,
                        &mut primary,
                        &mut contract_imports,
                    );
                }
            }
        } else {
            out_decls.push(decl.clone());
            primary.push(true);
        }
    }

    Ok(ExpandedProgram {
        program: Program { decls: out_decls },
        primary,
        contract_imports,
        ordered_modules,
        root_sla_modules,
        body_fallback_modules: table.body_fallback_modules.clone(),
    })
}

#[cfg(test)]
mod tests;
