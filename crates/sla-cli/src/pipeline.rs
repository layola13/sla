//! Compile pipeline — the mirror of `plugin_compile.zig`'s
//! shared front-end (`runSlaFrontend`) and tails
//! (`compileSlaToSaStringWithOptions` /
//! `compileSlaFileToSabWithOptions`), and of the `check`
//! command's inline twin in plugin_commands.zig (FB-038's dual
//! path: the two entry kinds stay visibly separate here, as in
//! the original).
//!
//! Stage order: freshness load → read → parse (with the D-P3
//! seed prescan) → [TestCodegen only: .sla import BODY
//! expansion (sla_plugin::sla_expand, include-all with bodies
//! kept — phase-6 census R-2), then filter prune +
//! reachability prune, the CLI-D5 stages in crate::test_prune]
//! → contract load + extern registration → type check → tail
//! codegen → freshness store. Stages with no port in
//! the existing crates — source_expand (@expand_tuple), .sla
//! import body expansion on the Check/Compile kinds, defer
//! desugar, monomorphizer, syntax lowering (go/chan/select),
//! the SAB audit, imported-function alias registration, and
//! output import-path rewriting — are NOT silently skipped:
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
use sla_ast::node::Node;
use sla_checker::Checker;
use std::io::Write;
use std::path::Path;
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
    let mut types: Vec<String> = Vec::new();
    let mut enums: Vec<String> = Vec::new();
    let base_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    for line in source.lines() {
        let Some(path) = import_path_of_line(line) else {
            continue;
        };
        if !path.ends_with(".sla") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(base_dir.join(&path)) else {
            continue;
        };
        for line in text.lines() {
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
                "struct" => &mut types,
                "enum" => &mut enums,
                _ => continue,
            };
            if !bucket.iter().any(|n| n == name) {
                bucket.push(name.to_string());
            }
        }
    }
    (types, enums)
}

fn import_path_of_line(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line.strip_prefix("@import")?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
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
    let content = match std::fs::read(file) {
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

    // Test-codegen .sla import BODY expansion (phase-6 census
    // R-2): on the test path the Zig front-end checks the program
    // with imported module bodies present — `sla test` bodies
    // call imported module functions, and a decl-only surface
    // leaves every such call undefined. The expansion runs the
    // include-all combination with bodies kept
    // (sla_plugin::sla_expand), BEFORE the CLI-D5 prunes so the
    // reachability prune sees (and keeps) the module functions
    // the tests actually reach. Other kinds are untouched: their
    // stage order is byte-identical to before. The expanded
    // modules join the freshness closure below.
    let mut expanded_modules: Vec<String> = Vec::new();
    if kind == FrontendKind::TestCodegen {
        let Node::Program(parsed) = &root else {
            return None;
        };
        let start = Instant::now();
        let options = sla_plugin::sla_expand::SlaImportExpansionOptions {
            include_all_imported_decls: true,
            imported_bodies_decl_only: false,
            ..Default::default()
        };
        match sla_plugin::sla_expand::expand_sla_imports(parsed, Path::new(file), &options) {
            Ok(expanded) => {
                expanded_modules = expanded.ordered_modules.clone();
                root = Node::Program(expanded.program);
            }
            Err(err) => {
                let _ = writeln!(
                    stderr,
                    "Import Error: failed to expand @import modules: {err}"
                );
                return None;
            }
        }
        stage_done(stderr, profile, "sla import body expansion", start);
    }

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
        crate::test_prune::prune_unreachable_test_function_decls_before_type_check(&mut root);
        stage_done(stderr, profile, "pre-typecheck reachable decl filter", start);
    }

    let Node::Program(program) = &root else {
        return None;
    };

    let start = Instant::now();
    let mut checker = Checker::new();
    let import_paths: Vec<String> = program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Node::ImportDecl(import) => Some(import.path.clone()),
            _ => None,
        })
        .collect();
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
            return None;
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
                        )
                    })
                    .collect::<Vec<_>>(),
                &func.ret_ty,
                func.return_fallible,
            ),
        ) {
            let zig_err = diag.kind.name();
            let _ = writeln!(
                stderr,
                "Type Check Error: failed to verify types: {} (error.{zig_err})",
                diag.message
            );
            return None;
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
                direct_callees: record.direct_callees.clone(),
            },
        );
    }
    stage_done(stderr, profile, &format!("{prefix}load contracts"), start);

    let start = Instant::now();
    if let Err(diag) = checker.check_program(program) {
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
        return None;
    }
    stage_done(stderr, profile, &format!("{prefix}type check"), start);

    let mut closure = vec![file.to_string()];
    closure.extend(expanded_modules.iter().cloned());
    closure.extend(report.visited.iter().cloned());
    match Path::new(file).parent() {
        Some(dir) => closure.push(dir.join("sa.mod").to_string_lossy().into_owned()),
        None => closure.push("sa.mod".to_string()),
    }

    Some(Frontend {
        root,
        checker,
        closure,
    })
}

// -- CheckInfo adapter (same wiring shape as sla-oracle's) -------------------------

struct CheckerInfo<'c>(&'c Checker);

impl sla_lowering::CheckInfo for CheckerInfo<'_> {
    fn expr_type(&self, node: &Node) -> Option<&sla_ast::Type> {
        self.0.expr_types.get(&sla_lowering::node_key(node))
    }
    fn cleanups(&self, stmt: &Node) -> Option<&[String]> {
        self.0
            .cleanups
            .get(&sla_lowering::node_key(stmt))
            .map(Vec::as_slice)
    }
    fn phi_cleanups(&self, stmt: &Node) -> Option<&[String]> {
        self.0
            .phi_cleanups
            .get(&sla_lowering::node_key(stmt))
            .map(Vec::as_slice)
    }
    fn type_is_copy_value(&self, ty: &sla_ast::Type) -> bool {
        self.0.type_is_copy(ty)
    }
    fn is_extern_func(&self, name: &str) -> bool {
        self.0.extern_funcs.contains_key(name)
    }
    fn is_macro(&self, name: &str) -> bool {
        self.0.macros.contains_key(name)
    }
}

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
    let info = CheckerInfo(&frontend.checker);
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
    let info = CheckerInfo(&frontend.checker);
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
    let pattern = filter?;
    if pattern.is_empty() {
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
    for decl in &program.decls {
        if let sla_ast::node::Node::TestDecl(test) = decl {
            if test.name.contains(pattern) {
                return Some(false);
            }
        }
    }
    Some(true)
}

#[cfg(test)]
mod tests;
