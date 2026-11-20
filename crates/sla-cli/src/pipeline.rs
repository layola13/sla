//! Compile pipeline — the mirror of `plugin_compile.zig`'s
//! shared front-end (`runSlaFrontend`) and tails
//! (`compileSlaToSaStringWithOptions` /
//! `compileSlaFileToSabWithOptions`), and of the `check`
//! command's inline twin in plugin_commands.zig (FB-038's dual
//! path: the two entry kinds stay visibly separate here, as in
//! the original).
//!
//! Stage order: freshness load → read → parse (with the D-P3
//! seed prescan) → [.sla import expansion
//! (sla_plugin::sla_expand, include-all: bodies kept on
//! TestCodegen — phase-6 census R-2 — and on Compile — A1
//! line; decl-only on Check, the `sla check` combination),
//! then the CLI-D5 prunes in crate::test_prune (TestCodegen:
//! filter + reachability; Compile: imported-body
//! reachability, rooted at the root file's decls)]
//! → monomorphize (sla_ast::monomorphize, FB-012; after the
//! prunes, mirroring the Zig expand → test-filter →
//! monomorphize → load-contracts order) → contract load +
//! extern registration + IMP-4 imported-
//! signature registration (crate::pipeline::expand_wire) →
//! type check → tail
//! codegen → freshness store. Stages with no port in
//! the existing crates — source_expand (@expand_tuple), the
//! reachability-selective expansion mode (IMP-1; Compile
//! runs the bodies-kept combination instead, see the
//! expansion site), defer desugar, syntax
//! (go/chan/select), the SAB audit, the IMP-4 alias metadata
//! table, and output import-path rewriting — are NOT silently
//! skipped:
//! programs needing them either fail loudly in a downstream
//! ported stage or diverge, and every such surface is
//! registered in docs/divergences/cli-commands.md. The
//! front-end closure used for the freshness store is
//! (root file + expanded module files + contract-load visited
//! files + sibling sa.mod), the closest counterpart of Zig's
//! (root + module-table keys + contract imports + sa.mod);
//! composition differences are registered there too.

use crate::freshness::{CompileOptions, FreshKind};
use crate::out::zig_error_name;
use checker_info::CheckerInfo;
use sla_ast::node::Node;
use sla_checker::Checker;
use std::io::Write;
use sla_parser::imports::{import_path_from_line, resolve_import_files};
use std::path::{Path, PathBuf};
use std::time::Instant;

const MAX_SOURCE_BYTES: usize = 10 * 1024 * 1024;

// -- SLA_PROFILE --------------------------------------------------------------

pub fn profile_enabled() -> bool {
    match std::env::var("SLA_PROFILE") {
        Ok(v) => !v.is_empty() && v != "0" && v != "false",
        Err(_) => false,
    }
}

fn stage_done(stderr: &mut impl Write, enabled: bool, label: &str, start: Instant) {
    if enabled {
        let _ = writeln!(stderr, "[sla-profile] {label}: {}ms", start.elapsed().as_millis());
    }
}

// -- seed prescan (D-P3 duty, same line-scan as sla-oracle) ---------------------

fn prescan_sla_imports(source_path: &Path, source: &str) -> (Vec<String>, Vec<String>) {
    // Zig `prescanSlaImportTypes` (D-P3): the root parse seeds
    // struct/enum names from every `.sla` file reachable through
    // the root's @import closure — quoted or bare paths, glob
    // patterns expanded through the same resolver the expansion
    // stage uses. The previous direct-read form silently skipped
    // glob imports (read_to_string on a `*.sla` path fails) and
    // bare import lines, so wildcard-imported struct names were
    // invisible at parse time and their literals failed with
    // SyntaxError.
    use std::collections::HashSet;
    let mut types: Vec<String> = Vec::new();
    let mut enums: Vec<String> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let base_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    let mut queue: Vec<(PathBuf, String)> =
        vec![(base_dir.to_path_buf(), source.to_string())];
    while let Some((dir, src)) = queue.pop() {
        for line in src.split('\n') {
            let Some(child) = import_path_from_line(line) else {
                continue;
            };
            if !child.ends_with(".sla") {
                continue;
            }
            if let Ok(files) = resolve_import_files(&dir, &child, None) {
                for file in files {
                    if !visited.insert(file.path.clone()) {
                        continue;
                    }
                    collect_prescan_type_names(&file.source, &mut types, &mut enums);
                    let child_dir = Path::new(&file.path)
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| dir.clone());
                    queue.push((child_dir, file.source.clone()));
                }
            }
        }
    }
    (types, enums)
}

fn collect_prescan_type_names(source: &str, types: &mut Vec<String>, enums: &mut Vec<String>) {
    for line in source.lines() {
        let mut words = line.trim_start().split_whitespace();
        let mut kw = words.next().unwrap_or("");
        if kw == "pub" {
            kw = words.next().unwrap_or("");
        }
        let name = words.next().unwrap_or("");
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let bucket = match kw {
            "struct" => &mut *types,
            "enum" => &mut *enums,
            _ => continue,
        };
        if !bucket.iter().any(|n| n == name) {
            bucket.push(name.to_string());
        }
    }
}

// -- parse diagnostics (parser.zig printDiagnostic) ------------------------------

/// Render the Zig parser diagnostic: header line, source window
/// (two lines either side), caret line with found/expected. The
/// brace-note tails depend on parser-internal state the Rust
/// parser does not expose; their absence is registered.
fn print_parse_diagnostic(
    stderr: &mut impl Write,
    file: &str,
    source: &str,
    err: &sla_parser::ParseError,
) {
    let (line_no, col_no) = line_col_of(source, err.start);
    let _ = writeln!(
        stderr,
        "Syntax Error: failed to parse {file}:{line_no}:{col_no}: error.{}",
        err.kind.name()
    );
    let _ = writeln!(stderr, "  |");
    let lines: Vec<&str> = source.split('\n').collect();
    let first_line = if line_no > 2 { line_no - 2 } else { 1 };
    let mut ln = first_line;
    while ln <= line_no + 2 {
        let Some(text) = lines.get(ln - 1) else {
            if ln > line_no {
                break;
            }
            ln += 1;
            continue;
        };
        let _ = writeln!(stderr, "{ln:5} | {text}");
        if ln == line_no {
            let _ = write!(stderr, "      | ");
            for _ in 1..col_no {
                let _ = write!(stderr, " ");
            }
            let found = found_token_text(source, err);
            let expected = err.expected.as_deref().unwrap_or("token");
            let _ = writeln!(stderr, "^ found '{found}', expected {expected}");
        }
        ln += 1;
    }
}

fn line_col_of(source: &str, offset: usize) -> (usize, usize) {
    let bytes = source.as_bytes();
    let mut line = 1usize;
    let mut line_start = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if i >= offset {
            break;
        }
        if b == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    (line, offset - line_start + 1)
}

fn found_token_text(source: &str, err: &sla_parser::ParseError) -> String {
    let bytes = source.as_bytes();
    if err.start >= bytes.len() {
        return "end of file".to_string();
    }
    let end = err.end.min(bytes.len());
    if end > err.start {
        return String::from_utf8_lossy(&bytes[err.start..end]).into_owned();
    }
    let b = bytes[err.start];
    if b.is_ascii_graphic() {
        return (b as char).to_string();
    }
    "end of file".to_string()
}

// -- the front-end -----------------------------------------------------------------

pub struct Frontend {
    pub root: Node,
    pub checker: Checker,
    /// Dependency closure for the freshness store: root file,
    /// then the contract closure's visited files, then the
    /// sibling sa.mod path (pushed unconditionally, as in Zig —
    /// an unreadable entry simply never lands in the manifest).
    pub closure: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FrontendKind {
    /// plugin_compile.zig runSlaFrontend (build / SAB tails).
    Compile,
    /// plugin_commands.zig check branch (inline twin, FB-038).
    Check,
    /// Test-input compiles: test bodies are parsed
    /// (prune_for_test_codegen), unlike the two kinds above.
    TestCodegen,
}

/// Run the mirrored front-end. None = a diagnostic was printed
/// and the caller returns 1. `test_filter` is the `sla test`
/// --filter pattern; it only takes effect for
/// [`FrontendKind::TestCodegen`], whose pruning stages run here —
/// after parse, before contract load and the type check — in the
/// Zig order (CLI-D5; see crate::test_prune).
pub fn run_frontend(
    file: &str,
    kind: FrontendKind,
    test_filter: Option<&str>,
    stderr: &mut impl Write,
) -> Option<Frontend> {
    let profile = profile_enabled();
    let prefix = if kind == FrontendKind::Check { "check " } else { "" };

    let start = Instant::now();
    let mut content = match std::fs::read(file) {
        Ok(bytes) => {
            if bytes.len() > MAX_SOURCE_BYTES {
                let _ = writeln!(
                    stderr,
                    "Error: failed to read file {file}: error.FileTooBig"
                );
                return None;
            }
            match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    let _ = writeln!(
                        stderr,
                        "Error: failed to read file {file}: error.InvalidUtf8"
                    );
                    return None;
                }
            }
        }
        Err(err) => {
            let _ = writeln!(
                stderr,
                "Error: failed to read file {file}: error.{}",
                zig_error_name(&err)
            );
            return None;
        }
    };
    stage_done(stderr, profile, &format!("{prefix}read source"), start);

    // R-3 (restored): @expand_tuple source expansion runs BEFORE
    // prescan/parse — the Zig front-end expands the root source,
    // so the parser never sees the directive. An invalid
    // directive fails loudly as error.InvalidExpandTuple. (This
    // wiring was the expand_tuple slice's core change; it must
    // not be dropped — the corpus expand_tuple family is its
    // regression anchor.)
    match sla_parser::source_expand::expand(&content) {
        Ok(expanded) => content = expanded,
        Err(_) => {
            let _ = writeln!(
                stderr,
                "Error: failed to expand {file}: error.InvalidExpandTuple"
            );
            return None;
        }
    }

    let start = Instant::now();
    let (seed_types, seed_enums) = prescan_sla_imports(Path::new(file), &content);
    let options = sla_parser::ParserOptions {
        parse_test_bodies: kind == FrontendKind::TestCodegen,
        ..Default::default()
    };
    let mut parser = sla_parser::Parser::with_options(&content, options);
    let type_refs: Vec<&str> = seed_types.iter().map(String::as_str).collect();
    let enum_refs: Vec<&str> = seed_enums.iter().map(String::as_str).collect();
    parser.seed_known_type_names(&type_refs, &enum_refs);
    let mut root = match parser.parse_program() {
        Ok(root) => root,
        Err(err) => {
            print_parse_diagnostic(stderr, file, &content, &err);
            return None;
        }
    };
    stage_done(stderr, profile, &format!("{prefix}parse"), start);

    // Auto-match std imports (LINE-AUTOMATCH-IMPL): the stage
    // below runs one expansion -> registration -> check pass.
    // When its candidate scan (or an undefined-name diagnostic)
    // finds std modules to import, it returns Retry and this
    // loop re-runs the stage with those modules appended to the
    // root program's imports — the same road an explicit
    // `@import` takes, so expansion, contract load, signature
    // registration and the freshness closure all treat the
    // matched modules exactly like user-written imports. The
    // loop is bounded; its last round runs with the scan
    // disabled so the natural checker diagnostic surfaces.
    let base_dir = Path::new(file).parent().unwrap_or_else(|| Path::new("."));
    let loaded_index = crate::automatch::AutomatchIndex::load(base_dir);
    let mut extra_imports: Vec<String> = Vec::new();
    let mut round = 0usize;
    loop {
        let scan_enabled = round <= MAX_AUTOMATCH_ROUNDS;
        let (index_ref, index_path) = match &loaded_index {
            Some((index, path)) => (Some(index), Some(path.as_str())),
            None => (None, None),
        };
        match run_frontend_stage(
            file,
            kind,
            test_filter,
            &root,
            &extra_imports,
            index_ref,
            index_path,
            scan_enabled,
            profile,
            stderr,
        ) {
            StageOutcome::Done(frontend) => return Some(frontend),
            StageOutcome::Failed => return None,
            StageOutcome::Retry(matches) => {
                for (name, module) in &matches {
                    let _ = writeln!(
                        stderr,
                        "note: auto-matched '{name}' from \"{module}\" (auto-import)"
                    );
                    if !extra_imports.contains(module) {
                        extra_imports.push(module.clone());
                    }
                }
                round += 1;
            }
        }
    }
}

/// Bound on auto-match retry rounds (each round imports at
/// least one new module; real programs need a handful).
const MAX_AUTOMATCH_ROUNDS: usize = 8;

enum StageOutcome {
    Done(Frontend),
    /// (name, module) pairs to import before re-running.
    Retry(Vec<(String, String)>),
    Failed,
}

/// One expansion -> contract-load -> registration -> check
/// pass over the parsed root, with the auto-matched
/// `extra_imports` injected at the head of the root program's
/// declarations (where a user's own `@import` lines sit).
#[allow(clippy::too_many_arguments)]
fn run_frontend_stage(
    file: &str,
    kind: FrontendKind,
    test_filter: Option<&str>,
    parsed_root: &Node,
    extra_imports: &[String],
    index: Option<&crate::automatch::AutomatchIndex>,
    index_path: Option<&str>,
    scan_enabled: bool,
    profile: bool,
    stderr: &mut impl Write,
) -> StageOutcome {
    let prefix = if kind == FrontendKind::Check { "check " } else { "" };
    let mut root = parsed_root.clone();
    if !extra_imports.is_empty() {
        if let Node::Program(program) = &mut root {
            let mut decls: Vec<Node> =
                Vec::with_capacity(program.decls.len() + extra_imports.len());
            for path in extra_imports {
                if !program
                    .decls
                    .iter()
                    .any(|d| matches!(d, Node::ImportDecl(i) if &i.path == path))
                {
                    decls.push(Node::ImportDecl(sla_ast::ImportDecl {
                        path: path.clone(),
                    }));
                }
            }
            decls.extend(program.decls.iter().cloned());
            program.decls = decls;
        }
    }

    // Root import paths before expansion replaces them (the
    // contract loader below recurses through them).
    let root_import_paths: Vec<String> = match &root {
        Node::Program(p) => p.decls.iter().filter_map(|d| match d {
            Node::ImportDecl(i) => Some(i.path.clone()), _ => None }).collect(),
        _ => Vec::new(),
    };

    // .sla import expansion, include-all combination:
    //   * TestCodegen (phase-6 census R-2): bodies kept — on
    //     the test path the Zig front-end checks the program
    //     with imported module bodies present, because `sla
    //     test` bodies call imported module functions and a
    //     decl-only surface leaves every such call undefined.
    //     Runs BEFORE the CLI-D5 prunes so the reachability
    //     prune sees (and keeps) the module functions the
    //     tests actually reach.
    //   * Check (module-expand line): decl-only — the exact
    //     combination `sla check` runs in Zig
    //     (include_all_imported_decls +
    //     imported_bodies_decl_only): imported bodies never
    //     reach the checker, only their declarations.
    //   * Compile (A1 line, 2026-11-14): bodies kept. This
    //     kind previously ran NO expansion at all — its Zig
    //     combination is the reachability-selective
    //     expansion (IMP-1, still unwired) — so no imported
    //     signatures registered and every root-body call
    //     into an imported .sla module (the whole .sla std
    //     surface: Instant::now, redis_push_lit,
    //     sla_json_parse, ...) fell to "Undefined call" at
    //     check_program. The combination here is include-all
    //     plus the primary-rooted reachability prune below
    //     (test_prune::prune_unreachable_imported_function_decls_before_type_check):
    //     every body the selective pass would materialize
    //     for the root's decls is present and checked;
    //     unreached imported bodies are pruned before the
    //     check instead of being emitted as root bodies
    //     (the SAB tail dispatches per decl). NOT the
    //     decl-only fake IMP-1/IMP-4 warn against.
    // The expanded modules join the freshness closure
    // below; the root groups' .sla modules feed the IMP-4
    // registration in the sequence further down.
    let mut expanded_modules: Vec<String> = Vec::new();
    let mut root_sla_modules: Vec<sla_plugin::sla_expand::RootSlaModule> = Vec::new();
    let mut root_flags: Vec<bool> = Vec::new();
    if matches!(kind, FrontendKind::TestCodegen | FrontendKind::Check | FrontendKind::Compile) {
        let Node::Program(parsed) = &root else {
            return StageOutcome::Failed;
        };
        let start = Instant::now();
        let options = sla_plugin::sla_expand::SlaImportExpansionOptions {
            include_all_imported_decls: true,
            imported_bodies_decl_only: kind == FrontendKind::Check,
            ..Default::default()
        };
        match sla_plugin::sla_expand::expand_sla_imports(parsed, Path::new(file), &options) {
            Ok(expanded) => {
                expanded_modules = expanded.ordered_modules.clone();
                root_sla_modules = expanded.root_sla_modules;
                root_flags =
                    crate::test_prune::root_decl_flags(&parsed.decls, &expanded.program.decls);
                root = Node::Program(expanded.program);
            }
            Err(err) => {
                let _ = writeln!(
                    stderr,
                    "Import Error: failed to expand @import modules: {err}"
                );
                return StageOutcome::Failed;
            }
        }
        stage_done(stderr, profile, "sla import expansion", start);
    }

    // F-G5: defer desugar (defer_desugar.zig) — after import
    // expansion, before the prunes and the checker, so no
    // downstream stage ever sees a DeferStmt.
    if let Err(err) = sla_ast::defer_desugar::desugar_program(&mut root) {
        let _ = writeln!(
            stderr,
            "Error: failed to desugar defer statements: {err}"
        );
        return StageOutcome::Failed;
    }
    // CLI-D5 imported-macro reachability wiring (LINE-IMPORTED-3):
    // the prunes below mirror plugin_reachability.zig, whose
    // call_expr arm marks an imported macro's direct callees
    // reachable at the call site; that needs the contract
    // registry's name -> direct_callees map BEFORE the prune.
    // The authoritative contract load further down re-runs the
    // same loader over the same import union and adjudicates
    // errors at its pinned stage; this prune-local load is
    // reachability input only, so a load error leaves the map
    // empty (the prune then behaves exactly as it did before
    // this wiring) and the error still surfaces below, unchanged.
    let imported_macro_callees: std::collections::BTreeMap<String, Vec<String>> =
        if matches!(kind, FrontendKind::TestCodegen | FrontendKind::Compile) {
            let mut paths: Vec<String> = root_import_paths.clone();
            if let Node::Program(program) = &root {
                for decl in &program.decls {
                    if let Node::ImportDecl(import) = decl {
                        if !paths.contains(&import.path) {
                            paths.push(import.path.clone());
                        }
                    }
                }
            }
            match sla_parser::contract_load::load_imported_contracts(&paths, Path::new(file)) {
                Ok(report) => {
                    let mut map: std::collections::BTreeMap<String, Vec<String>> =
                        std::collections::BTreeMap::new();
                    for record in &report.imported_macro_registry {
                        map.entry(record.name.clone())
                            .or_insert_with(|| record.direct_callees.clone());
                    }
                    map
                }
                Err(_) => std::collections::BTreeMap::new(),
            }
        } else {
            std::collections::BTreeMap::new()
        };

    // Test-codegen pre-check pruning (plugin_compile.zig
    // runSlaFrontend): pruneTestsByFilter, then the syntactic
    // reachability prune — both BEFORE contract load and
    // checkProgram, so the checker's node-keyed cleanup tables
    // describe exactly the program codegen consumes (CLI-D5).
    // Other kinds never prune here; their stage order is
    // byte-identical to before.
    if kind == FrontendKind::TestCodegen {
        let start = Instant::now();
        crate::test_prune::prune_tests_by_filter(&mut root, test_filter);
        stage_done(stderr, profile, "test filter prune", start);
        let start = Instant::now();
        crate::test_prune::prune_unreachable_test_function_decls_before_type_check(
            &mut root,
            &imported_macro_callees,
        );
        stage_done(stderr, profile, "pre-typecheck reachable decl filter", start);
    }

    // Compile-kind pre-check prune (A1 line; other kinds
    // never prune here): drop unreached imported bodies.
    if kind == FrontendKind::Compile {
        let start = Instant::now();
        crate::test_prune::prune_unreachable_imported_function_decls_before_type_check(
            &mut root,
            &root_flags,
            &imported_macro_callees,
        );
        stage_done(stderr, profile, "compile reachable imported-body prune", start);
    }

    // FB-012: monomorphize (monomorphizer.zig via
    // plugin_compile.zig) — after the prunes, before contract
    // load + check_program, in the Zig stage order. The prune
    // counts GenericFuncRef / generic calls as reference edges
    // (test_reach.rs), so used templates are still present;
    // instances are appended as ordinary declarations and the
    // checker never sees a GenericFuncRef on this path (D-C11).
    // Error text is MonomorphizeError's Display, mirroring
    // plugin_compile.zig byte-for-byte.
    {
        let start = Instant::now();
        if let Err(err) = sla_ast::monomorphize::monomorphize_program(&mut root) {
            let _ = writeln!(stderr, "{err}");
            return StageOutcome::Failed;
        }
        stage_done(stderr, profile, &format!("{prefix}monomorphize"), start);
    }

    let Node::Program(program) = &root else {
        return StageOutcome::Failed;
    };

    let start = Instant::now();
    let mut checker = Checker::new();
    // Pre-expansion root imports UNION retained expanded
    // imports: the loader recurses through `.sla` modules
    // into pulled-in `.sa`/`.sai` files (macros/contracts).
    let mut import_paths: Vec<String> = root_import_paths;
    for decl in &program.decls {
        if let Node::ImportDecl(import) = decl {
            if !import_paths.contains(&import.path) {
                import_paths.push(import.path.clone());
            }
        }
    }
    // Auto-matched modules join the contract load exactly like
    // retained expanded imports (their `.sla` decls are already
    // in the program via the injected ImportDecls; this list is
    // what pulls their `.sai`/`.sa` contracts and macros in).
    for path in extra_imports {
        if !import_paths.contains(path) {
            import_paths.push(path.clone());
        }
    }
    let report = match sla_parser::contract_load::load_imported_contracts(
        &import_paths,
        Path::new(file),
    ) {
        Ok(report) => report,
        Err(err) => {
            let _ = writeln!(
                stderr,
                "Import Error: failed to load @import contracts: {err}"
            );
            return StageOutcome::Failed;
        }
    };
    for func in &report.externs {
        // R-6 follow-up: contracts register under the
        // registerExternContract discipline (identical duplicates
        // dedupe, distinct same-name declarations promote into
        // extern overload sets) instead of a bare insert that let
        // same-name pairs silently collapse.
        if let Err(diag) = checker.register_extern_contract(
            &func.name,
            sla_checker::check_overload3::extern_sig_from_parts(
                &func
                    .params
                    .iter()
                    .map(|pp| {
                        (
                            pp.name.clone(),
                            pp.ty.clone(),
                            pp.is_borrow,
                            pp.is_move,
                            pp.is_raw,
                        )
                    })
                    .collect::<Vec<_>>(),
                &func.ret_ty,
                func.return_fallible,
                func.is_variadic,
            ),
        ) {
            let zig_err = diag.kind.name();
            let _ = writeln!(
                stderr,
                "Type Check Error: failed to verify types: {} (error.{zig_err})",
                diag.message
            );
            return StageOutcome::Failed;
        }
    }
    // R-1 fix (all kinds) + R-2 replay note: imported macros
    // register too (mirrors the sla-oracle wiring). Without this
    // the CLI front end never populated `imported_macros`, so
    // every sa_std macro call (STR_PTR, BYTES_EQ, PTR_NULL,
    // VEC_RESERVE_GROW, ...) in any checked body fell to
    // "Undefined call" even though the checker's imported-macro
    // stage and the contract registry both exist. The R-2 line
    // scoped the same registration to the TestCodegen kind on
    // the pre-R-1 base; R-1's all-kind loop (already on this
    // head) is the superset, so the replay keeps exactly this
    // one loop — registering twice would be a no-op re-insert
    // of identical records, and the Check/Compile-kind gap the
    // R-2 batch deliberately left open is already closed here.
    for record in &report.imported_macro_registry {
        checker.register_imported_macro(
            &record.name,
            sla_checker::ImportedMacroInfo {
                arity: record.arity,
                leading_outputs: record.leading_outputs,
                import_path: record.import_path.clone(),
                borrowed_arg_mask: record.borrowed_arg_mask,
                address_slot_arg_mask: record.address_slot_arg_mask,
                assigned_output_mask: record.assigned_output_mask,
                direct_callees: record.direct_callees.clone(),
            },
        );
    }
    // IMP-4 (module-expand line): imported-function signature
    // registration — root .sla modules' exported functions
    // register under bare names and `namespace__name` aliases,
    // impl methods under mangled symbols (see
    // pipeline::expand_wire for the exact surface and the
    // unported metadata-table residual). Zig's order: after
    // contract loading, before checkProgram. A kind that ran
    // no expansion carries no root modules and skips this.
    if !root_sla_modules.is_empty() {
        expand_wire::register_imported_signatures(&mut checker, &root_sla_modules);
    }
    stage_done(stderr, profile, &format!("{prefix}load contracts"), start);

    // Auto-match scan (LINE-AUTOMATCH-IMPL): names the expanded
    // program references but nothing defines — not the program
    // itself, not an explicit import, not a builtin — are looked
    // up in the std index. A name with exactly one providing
    // module re-enters this stage with that module imported
    // (StageOutcome::Retry); names with two or more providers
    // are remembered here and reported loudly below if the
    // checker then proves the name really is unresolved.
    let mut ambiguous: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    if scan_enabled {
        if let Some(index) = index {
            let known = crate::automatch::known_names(&checker, program);
            let candidates = crate::automatch::collect_candidates(program, &known);
            if !candidates.is_empty() {
                let already_imported: std::collections::HashSet<String> =
                    import_paths.iter().cloned().collect();
                let (new_matches, amb) =
                    crate::automatch::plan_matches(index, &candidates, &already_imported);
                ambiguous = amb;
                if !new_matches.is_empty() {
                    return StageOutcome::Retry(new_matches);
                }
            }
        }
    }

    let start = Instant::now();
    if let Err(diag) = checker.check_program(program) {
        // Auto-match diagnostic path: when the checker proved a
        // name unresolved and the index settles it, act on that
        // instead of surfacing the bare undefined error — two or
        // more providers are a loud ambiguity error naming the
        // candidate modules; a unique provider the scan did not
        // reach retries with the module imported.
        if scan_enabled {
            if let Some(index) = index {
                if let Some(name) = crate::automatch::undefined_name(&diag) {
                    if let Some(modules) = ambiguous.get(&name) {
                        let _ = writeln!(
                            stderr,
                            "AutoMatch Error: '{name}' is declared by multiple std modules ({}); add an explicit @import to choose one",
                            modules.join(", ")
                        );
                        return StageOutcome::Failed;
                    }
                    let modules = index.modules_for(&name);
                    if modules.len() == 1 && !import_paths.contains(&modules[0]) {
                        return StageOutcome::Retry(vec![(name, modules[0].clone())]);
                    }
                }
            }
        }
        // Zig checkProgram returns the checker's own error and the
        // command layer prints its name verbatim; the Rust diagnostic
        // kinds carry the same identifiers (the message text already
        // matches byte-for-byte, e.g. the tail is error.TypeMismatch
        // for a return-type mismatch).
        let zig_err = diag.kind.name();
        let _ = writeln!(
            stderr,
            "Type Check Error: failed to verify types: {} (error.{zig_err})",
            diag.message
        );
        return StageOutcome::Failed;
    }
    stage_done(stderr, profile, &format!("{prefix}type check"), start);

    let mut closure = vec![file.to_string()];
    closure.extend(expanded_modules.iter().cloned());
    closure.extend(report.visited.iter().cloned());
    match Path::new(file).parent() {
        Some(dir) => closure.push(dir.join("sa.mod").to_string_lossy().into_owned()),
        None => closure.push("sa.mod".to_string()),
    }
    // The auto-match index joins the freshness closure: a std
    // update that changes the index must invalidate artifacts
    // whose compilation the index could have steered. (Matched
    // module files already join through expanded_modules and
    // the contract closure above, like explicit imports.)
    if let Some(index_path) = index_path {
        closure.push(index_path.to_string());
    }

    StageOutcome::Done(Frontend {
        root,
        checker,
        closure,
    })
}

// -- CheckInfo adapter: crate::pipeline::checker_info ---------------------------

// -- tails ---------------------------------------------------------------------------

/// plugin_compile.zig `compileSlaToSaStringWithOptions`, with the
/// freshness gate at both ends.
pub fn compile_to_sa_string(
    file: &str,
    kind: FrontendKind,
    options: &CompileOptions,
    output_file: &str,
    stderr: &mut impl Write,
) -> Option<String> {
    let profile = profile_enabled();
    if let Some(cached) =
        crate::freshness::try_load_fresh_artifact(file, FreshKind::SaText, options, Some(output_file))
    {
        if profile {
            let _ = writeln!(stderr, "[sla-profile] freshness gate: hit, front-end skipped");
        }
        return String::from_utf8(cached).ok();
    }
    // No post-check filter prune: in the Zig original
    // pruneTestsByFilter exists only pre-check (inside
    // runSlaFrontend), so the former post-check call here — the
    // CLI-D5 misalignment itself — is gone. (The SAB tail's
    // post-check reachability prunes are separate stages; see
    // the CLI-D5 scope note in docs/divergences/cli-commands.md.)
    let frontend = run_frontend(file, kind, options.test_filter.as_deref(), stderr)?;
    let info = CheckerInfo::new(&frontend.checker);
    let start = Instant::now();
    let text = match sla_codegen_text::generate(&frontend.root, &info) {
        Ok(text) => text,
        Err(err) => {
            let name = match &err {
                sla_codegen_text::TextError::Unsupported { .. } => "UnsupportedFeature",
                sla_codegen_text::TextError::MissingType { .. } => "MissingType",
                sla_codegen_text::TextError::Lowering(_) => "LoweringError",
            };
            // As in the SAB tail: append the error's Display detail
            // after the variant name instead of dropping the payload
            // (feature/context). The old line stays a strict prefix.
            let _ = writeln!(
                stderr,
                "Codegen Error: failed to generate SA code: error.{name}: {err}"
            );
            return None;
        }
    };
    stage_done(stderr, profile, "sa codegen", start);
    crate::freshness::store_fresh_artifact(
        file,
        FreshKind::SaText,
        options,
        Some(output_file),
        &frontend.closure,
        text.as_bytes(),
    );
    Some(text)
}

/// plugin_compile.zig `compileSlaFileToSabWithOptions`, with the
/// freshness gate at both ends.
pub fn compile_to_sab(
    file: &str,
    kind: FrontendKind,
    options: &CompileOptions,
    stderr: &mut impl Write,
) -> Option<Vec<u8>> {
    let profile = profile_enabled();
    if let Some(cached) =
        crate::freshness::try_load_fresh_artifact(file, FreshKind::Sab, options, None)
    {
        if profile {
            let _ = writeln!(stderr, "[sla-profile] freshness gate: hit, front-end skipped");
        }
        return Some(cached);
    }
    let frontend = run_frontend(file, kind, options.test_filter.as_deref(), stderr)?;
    let info = CheckerInfo::new(&frontend.checker);
    let start = Instant::now();
    // CLI-D4: the SAB tail runs through the F-group entry with a
    // real StdSnippetFlattener provider — the host `sa` reached
    // through the delegation seam (sa_flattener.rs). Std-surface
    // programs (println & friends) therefore lower directly
    // instead of stopping at the SAB-D31 boundary; a host that
    // cannot serve a snippet fails loudly through the provider.
    let flattener = Box::new(crate::sa_flattener::SaHostFlattener::new(
        crate::delegate::resolve_sa_executable(),
    ));
    let bytes = match sla_codegen_sab::generate_from_program_with_flattener(
        &frontend.root,
        &info,
        sla_codegen_sab::surface_load::SurfaceRoots::from_env(),
        flattener,
    ) {
        Ok(bytes) => bytes,
        Err(err) => {
            // The SAB error's Display renders `{Variant}: {detail}`;
            // printing `error.{err}` surfaces the SAB-D11 payload
            // (construct names, flattener host stderr) that the old
            // name-only form dropped. The old line stays a strict
            // prefix of this one.
            let _ = writeln!(
                stderr,
                "SAB Error: direct SLA-to-SAB lowering failed: error.{err}"
            );
            return None;
        }
    };
    stage_done(stderr, profile, "sab direct codegen", start);
    crate::freshness::store_fresh_artifact(
        file,
        FreshKind::Sab,
        options,
        None,
        &frontend.closure,
        &bytes,
    );
    Some(bytes)
}

/// Zig `testFilterSelectsNoTests` (plugin_compile.zig): probe
/// parse of the root file; Some(true) when a non-empty filter
/// matches no @test name. Probe errors print like the original
/// and yield None (the compile then proceeds and reports again).
/// The Zig probe expands @import'ed modules before scanning;
/// this port scans the root program only — registered.
pub fn test_filter_selects_no_tests(file: &str, filter: Option<&str>, stderr: &mut impl Write) -> Option<bool> {
    let pattern = filter.unwrap_or("");
    if filter.is_some() && pattern.is_empty() {
        return None;
    }
    let content = match std::fs::read(file) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => return None,
        },
        Err(err) => {
            let _ = writeln!(
                stderr,
                "Error: failed to read file {file}: error.{}",
                zig_error_name(&err)
            );
            return None;
        }
    };
    let options = sla_parser::ParserOptions {
        parse_test_bodies: false,
        ..Default::default()
    };
    let mut parser = sla_parser::Parser::with_options(&content, options);
    let root = parser.parse_program().ok()?;
    let sla_ast::node::Node::Program(program) = &root else {
        return None;
    };
    let mut any_test = false;
    for decl in &program.decls {
        if let sla_ast::node::Node::TestDecl(test) = decl {
            any_test = true;
            if !pattern.is_empty() && test.name.contains(pattern) {
                return Some(false);
            }
        }
    }
    if filter.is_none() {
        // No filter: "selects no tests" only when the file
        // declares no @test at all. The Zig frontend prints the
        // 0-passed ok line for such files without building a
        // test binary; building one would link every declared
        // function, including wrappers over provider-less
        // externs in contract fixtures (nested-contract /
        // view-args shape), and fail at the host link step.
        return Some(!any_test);
    }
    Some(true)
}

mod checker_info;
mod expand_wire;

#[cfg(test)]
mod tests;
