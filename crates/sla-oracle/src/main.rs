//! sla-oracle: dev-only differential harness (six-house verdict 2026-11-07,
//! mandatory item 5). Nobody depends on this crate.
//!
//! Duties:
//! - run the Rust pipeline over fixtures/ and byte-compare against the Zig
//!   plugin's expected outputs (fixtures/expected/) — via scripts/compare.sh;
//! - emit a NORMALIZED BOUNDARY DUMP at every crate boundary so a divergence
//!   can be attributed to one stage instead of only showing up as a final
//!   byte diff: seed / tokens / AST / checked / lowering / SAB bytes;
//! - own the D-P3 seed duty: `.sla` imports contribute only module names
//!   inside the parser (it does no filesystem IO); the struct/enum names
//!   of imported `.sla` files are prescanned HERE and injected through
//!   `Parser::seed_known_type_names`, the explicit replacement for the
//!   Zig parser's in-parser prescan (divergences D-P3 / D-P11);
//! - staged timing (SLA_RS_PROFILE=1 prints per-stage ms to stderr).
//!
//! Usage: sla-oracle <input.sla> [-o out.sa] [--dump-boundaries <dir>]
//!        [--build-mode | --no-build-mode]
//!
//! Build mode (the default, D-O2): after parsing and before checking,
//! every top-level `@test` body is emptied in place, mirroring the Zig
//! pipeline, whose build mode clears test bodies before codegen so the
//! `sa sla build` artifact carries only the test shell (header, entry
//! label, `return`). `--no-build-mode` (alias `--keep-test-bodies`)
//! keeps the full bodies for diagnosis; compare/bench runs must use
//! the default so they measure the same program the plugin builds.
//!
//! Contracts stage (INT-2): between pruning and checking, the
//! program's `@import` contract closure is loaded through the ONE
//! shared machinery (`sla_parser::contract_load`, the
//! loadImportedContracts mirror) and every `.sai` extern is
//! registered into the checker — the same registration the plugin
//! wiring layer performs (`sla_plugin::contracts`), consumed here
//! directly because the frozen Cargo graph gives this crate no edge
//! to sla-plugin. A load failure is fatal (exit 5), mirroring the
//! Zig plugin's "Import Error: failed to load @import contracts".
//!
//! Exit codes: 0 full pipeline produced output; 2 usage / IO error;
//! 3 text codegen failed loudly (unported emission path) after
//! successful parse+check; 4 parse error (including root-source
//! `@expand_tuple` expansion failure, which precedes parsing);
//! 5 check diagnostic.
//! Stages that do not exist yet dump an explicit NOT-IMPLEMENTED marker
//! instead of fabricated data — an oracle that invents boundary data is
//! worse than one that admits the gap.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use sla_ast::node::Node;
use sla_checker::Checker;
use sla_lexer::{Lexer, Tag};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut dump_dir: Option<PathBuf> = None;
    // Build mode is the default (D-O2): the oracle's compare/bench
    // paths are calibrated against `sa sla build` artifacts, which
    // carry emptied @test bodies, so keeping bodies by default would
    // compare a different program than the plugin builds.
    let mut build_mode = true;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                output = args.get(i).map(PathBuf::from);
            }
            "--dump-boundaries" => {
                i += 1;
                dump_dir = args.get(i).map(PathBuf::from);
            }
            "--build-mode" => build_mode = true,
            "--no-build-mode" | "--keep-test-bodies" => build_mode = false,
            other => input = Some(PathBuf::from(other)),
        }
        i += 1;
    }
    let Some(input) = input else {
        eprintln!(
            "usage: sla-oracle <input.sla> [-o out.sa] [--dump-boundaries <dir>] \
             [--build-mode | --no-build-mode]"
        );
        return ExitCode::from(2);
    };
    let profile = std::env::var_os("SLA_RS_PROFILE").is_some();

    let source = match std::fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sla-oracle: cannot read {}: {e}", input.display());
            return ExitCode::from(2);
        }
    };
    // -- stage: source expansion (phase 4): Zig plugin_compile
    // expands the ROOT source before parsing (expandForModulePath);
    // an invalid directive is the "Macro Expansion Error" path.
    // Exit 4 (the parse family): expansion is the first transform.
    let source =
        match sla_parser::source_expand::expand_for_module_path(&input.to_string_lossy(), &source) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("sla-oracle: source expansion failed: {e}");
                return ExitCode::from(4);
            }
        };
    if let Some(dir) = &dump_dir {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("sla-oracle: cannot create dump dir {}: {e}", dir.display());
            return ExitCode::from(2);
        }
    }
    let dump = |name: &str, text: &str| {
        if let Some(dir) = &dump_dir {
            let path = dir.join(name);
            if let Err(e) = std::fs::write(&path, text) {
                eprintln!("sla-oracle: cannot write {}: {e}", path.display());
            }
        }
    };

    // -- stage: seed (D-P3 duty, coordinator-owned prescan) ----------------
    let t = Instant::now();
    let (seed_types, seed_enums, seed_notes) = prescan_sla_imports(&input, &source);
    stage_done(profile, "seed", t);
    let mut seed_text = String::from("# D-P3 seed (oracle prescan of .sla imports)\n");
    for note in &seed_notes {
        seed_text.push_str(&format!("# {note}\n"));
    }
    seed_text.push_str(&format!("types: {}\n", seed_types.join(", ")));
    seed_text.push_str(&format!("enums: {}\n", seed_enums.join(", ")));
    dump("00_seed.txt", &seed_text);

    // -- stage: tokens ------------------------------------------------------
    let t = Instant::now();
    let mut tokens_text = String::new();
    {
        let mut lx = Lexer::new(source.as_bytes());
        let mut idx = 0usize;
        loop {
            let tok = lx.next();
            let text = escape_bytes(lx.token_text(tok));
            tokens_text.push_str(&format!(
                "{idx} {:?} {start} {end} {text}\n",
                tok.tag,
                start = tok.loc.start,
                end = tok.loc.end
            ));
            idx += 1;
            if tok.tag == Tag::Eof || idx > 1_000_000 {
                break;
            }
        }
    }
    stage_done(profile, "lex", t);
    dump("01_tokens.txt", &tokens_text);

    // -- stage: parse (with seed) -------------------------------------------
    let t = Instant::now();
    let mut parser = sla_parser::Parser::new(&source);
    let type_refs: Vec<&str> = seed_types.iter().map(String::as_str).collect();
    let enum_refs: Vec<&str> = seed_enums.iter().map(String::as_str).collect();
    parser.seed_known_type_names(&type_refs, &enum_refs);
    let mut root = match parser.parse_program() {
        Ok(root) => root,
        Err(e) => {
            let expected = e.expected.as_deref().unwrap_or("-");
            eprintln!(
                "sla-oracle: parse error {:?} at {}..{}: {}",
                e.kind, e.start, e.end, expected
            );
            dump(
                "02_ast.txt",
                &format!("PARSE ERROR {:?} at {}..{}: {}\n", e.kind, e.start, e.end, expected),
            );
            return ExitCode::from(4);
        }
    };
    stage_done(profile, "parse", t);

    // -- stage: build-mode @test pruning (D-O2) ---------------------------
    // The Zig pipeline's build mode clears @test bodies before codegen
    // (plugin_compile; D-T7 / FB-008): the build artifact keeps only
    // the declaration shell — name and ignored/should_panic modifiers —
    // and codegen's gen_test_decl emits header + entry label + `return`
    // for the empty body, exactly the golden shape in fixtures/expected.
    // The pruning therefore replaces each TestDecl's statement list
    // with an empty one, in place: the tree is NOT cloned or rebuilt,
    // so the surviving nodes keep their addresses for the checker's
    // node-key side tables (D-T10), and it runs before checking so the
    // checker and codegen both see the build-mode program. The AST
    // dump below is this pruned tree — what the later stages consume.
    let t = Instant::now();
    let pruned_tests = if build_mode {
        prune_test_bodies(&mut root)
    } else {
        0
    };
    stage_done(profile, "build_mode_prune", t);

    dump("02_ast.txt", &format!("{root:#?}\n"));

    // -- stage: contracts (INT-2) ---------------------------------------------
    // Load the @import contract closure and register its externs
    // before checking (Zig: loadImportedContracts runs ahead of
    // checkProgram). The dump lists everything the load touched —
    // including the parts that have no registration home yet
    // (layout defines, INT2-3) — loudly, never silently. Imported
    // macros register in this stage since phase 4; .sla body
    // expansion remains a separate plugin-layer stage
    // (sla_plugin::sla_expand, include-all mode).
    let t = Instant::now();
    let Node::Program(program) = &root else {
        eprintln!("sla-oracle: parser root is not a Program node");
        return ExitCode::from(3);
    };
    let mut checker = Checker::new();
    let import_paths = sla_plugin_contract_paths(program);
    match sla_parser::contract_load::load_imported_contracts(&import_paths, &input) {
        Ok(report) => {
            for func in &report.externs {
                // R-6 follow-up: register under the
                // registerExternContract discipline (dedupe /
                // promote) with parameter spellings carried.
                let _ = checker.register_extern_contract(
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
                );
            }
            // Phase 4 (INT2-4 closed): imported macros register too.
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
            dump("03a_contracts.txt", &contracts_dump_text(&report));
        }
        Err(e) => {
            eprintln!("sla-oracle: contract load failed: {e}");
            dump(
                "03a_contracts.txt",
                &format!("# INT-2 contract load\nCONTRACT LOAD ERROR: {e}\n"),
            );
            return ExitCode::from(5);
        }
    }
    stage_done(profile, "contracts", t);

    // -- stage: check ---------------------------------------------------------
    let t = Instant::now();
    let mut checked_text = String::new();
    // Dump caliber (D-O2): in build mode the checker saw emptied @test
    // bodies; say so on the checked dump so a missing test-body fact is
    // read as the build-mode program, not as a checker gap.
    if build_mode {
        checked_text.push_str(&format!(
            "mode: build (@test bodies pruned before check: {pruned_tests})\n"
        ));
    } else {
        checked_text.push_str("mode: full (--no-build-mode: @test bodies kept)\n");
    }
    let check_rc = match checker.check_program(program) {
        Ok(()) => {
            checked_text.push_str("OK\n");
            0
        }
        Err(d) => {
            checked_text.push_str(&format!(
                "DIAGNOSTIC kind={} marker={} message={}\n",
                d.kind.name(),
                d.marker.unwrap_or("-"),
                d.message
            ));
            5
        }
    };
    let mut struct_names: Vec<&String> = checker.structs.keys().collect();
    struct_names.sort();
    let mut enum_names: Vec<&String> = checker.enums.keys().collect();
    enum_names.sort();
    checked_text.push_str(&format!(
        "structs({}): {}\n",
        struct_names.len(),
        struct_names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    checked_text.push_str(&format!(
        "enums({}): {}\n",
        enum_names.len(),
        enum_names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    stage_done(profile, "check", t);
    dump("03_checked.txt", &checked_text);
    if check_rc != 0 {
        return ExitCode::from(5);
    }

    // -- stage: lowering / text codegen ---------------------------------------
    // The lowering rules are consumed inside sla-codegen-text through the
    // CheckInfo surface; the standalone reachable-set dump lands when
    // sla-lowering's reachability implementation does (the contract type
    // exists, the computation is the lowering lane's).
    dump(
        "04_lowering.txt",
        "CONSUMED-INTERNAL: lowering facts flow into codegen-text via the\n\
         CheckInfo adapter (oracle, below); no separate lowered-form dump\n\
         exists yet. ReachableSet computation: pending lowering lane.\n",
    );
    let t = Instant::now();
    let info = CheckerInfo(&checker);
    let generated = sla_codegen_text::generate(&root, &info);
    stage_done(profile, "codegen_text", t);
    let text = match generated {
        Ok(text) => text,
        Err(e) => {
            eprintln!("sla-oracle: text codegen failed loudly: {e}");
            dump("06_text.txt", &format!("CODEGEN ERROR {e}\n"));
            return ExitCode::from(3);
        }
    };
    dump("06_text.txt", &text);
    if let Some(out_path) = &output {
        if let Err(e) = std::fs::write(out_path, &text) {
            eprintln!("sla-oracle: cannot write {}: {e}", out_path.display());
            return ExitCode::from(2);
        }
    }

    // -- boundary artifact available today: SAB model bytes -------------------
    // The SAB lane's hand-lowered fixture01 model, encoded by the ported
    // sci encoder. Model-level (not AST-driven) per-function bytes; the
    // AST-driven SAB boundary lands with the lowering lane.
    match sla_codegen_sab::fixture01::fixture01_module() {
        module => match sla_codegen_sab::generate(&module) {
            Ok(bytes) => {
                let mut text =
                    format!("# fixture01 model SAB bytes (model-level, not AST-driven)\nlen {}\n", bytes.len());
                for (off, chunk) in bytes.chunks(16).enumerate() {
                    text.push_str(&format!("{off:04x}: "));
                    for b in chunk {
                        text.push_str(&format!("{b:02x} "));
                    }
                    text.push('\n');
                }
                dump("05_sab_fixture01.txt", &text);
            }
            Err(e) => dump("05_sab_fixture01.txt", &format!("ENCODE ERROR {e:?}\n")),
        },
    }

    ExitCode::from(0)
}

/// Adapter from the checker's side tables to sla-lowering's `CheckInfo`
/// surface (the integration wiring between lanes C and D). Keys are node
/// addresses via `sla_lowering::node_key`, the same computation the
/// checker used when filling the tables (`node_key_of` there).
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

    // `func` keeps the trait default (None): the checker's registry
    // holds FuncSig, not the FuncDecl the trait hands out; codegen
    // walks the program node for declarations. If a fixture shows this
    // default costing output, it becomes a registered divergence, not
    // a silent workaround.
}

/// The program's `@import` paths in declaration order — the same
/// extraction the plugin wiring performs
/// (`sla_plugin::contracts::decl_import_paths`); the load itself is
/// the shared `sla_parser::contract_load` machinery, so this is a
/// call-site shim, not a second expansion implementation.
fn sla_plugin_contract_paths(program: &sla_ast::Program) -> Vec<String> {
    program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Node::ImportDecl(import) => Some(import.path.clone()),
            _ => None,
        })
        .collect()
}

/// Boundary dump for the contracts stage: everything the load did,
/// with the not-yet-registrable parts named as such.
fn contracts_dump_text(report: &sla_parser::contract_load::LoadReport) -> String {
    let mut text = String::from("# INT-2 contract load (loadImportedContracts mirror)\n");
    text.push_str(&format!("visited ({}):\n", report.visited.len()));
    for path in &report.visited {
        text.push_str(&format!("  {path}\n"));
    }
    text.push_str(&format!(
        "registered externs ({}):\n",
        report.externs.len()
    ));
    for func in &report.externs {
        let params: Vec<String> = func
            .params
            .iter()
            .map(|p| {
                let prefix = if p.is_borrow {
                    "&"
                } else if p.is_move {
                    "^"
                } else {
                    ""
                };
                format!("{prefix}{}: {}", p.name, p.ty)
            })
            .collect();
        let fallible = if func.return_fallible { "!" } else { "" };
        text.push_str(&format!(
            "  {}({}) -> {}{}\n",
            func.name,
            params.join(", "),
            func.ret_ty,
            fallible
        ));
    }
    text.push_str(&format!(
        "layout defines ({}) [NO CHECKER TABLE - INT2-3]:\n",
        report.layout_defines.len()
    ));
    for def in &report.layout_defines {
        text.push_str(&format!("  {} = {} ({})\n", def.name, def.val_str, def.val_int));
    }
    text.push_str(&format!(
        "macro defs by file ({} files):\n",
        report.macro_defs.len()
    ));
    for (file, names) in &report.macro_defs {
        text.push_str(&format!("  {file}: {}\n", names.join(", ")));
    }
    text.push_str(&format!(
        "imported macros REGISTERED ({} in registry):\n",
        report.imported_macro_registry.len()
    ));
    for record in &report.imported_macro_registry {
        text.push_str(&format!(
            "  {} arity={} leading={} borrowed={:#x} addr={:#x} assigned={:#x} from={}\n",
            record.name,
            record.arity,
            record.leading_outputs,
            record.borrowed_arg_mask,
            record.address_slot_arg_mask,
            record.assigned_output_mask,
            record.import_path.as_deref().unwrap_or("-")
        ));
    }
    text.push_str(&format!(
        ".sla imports NOT body-expanded by the contracts stage ({}):\n",
        report.sla_imports.len()
    ));
    for path in &report.sla_imports {
        text.push_str(&format!("  {path}\n"));
    }
    text
}

/// D-O2 build-mode pruning: empty every top-level `TestDecl` body in
/// place, preserving the declaration shell (name, `is_ignored`,
/// `should_panic`). Returns how many bodies were non-empty and got
/// cleared. Only the body's own `Vec` is cleared — no node of the
/// surviving tree moves, so node-address keys stay valid (D-T10).
/// Tests live only at the top level of a program (the parser flattens
/// modules and has no other TestDecl nesting), so one pass over the
/// program's declarations is the whole operation.
fn prune_test_bodies(root: &mut Node) -> usize {
    let Node::Program(program) = root else {
        return 0;
    };
    let mut pruned = 0;
    for decl in &mut program.decls {
        if let Node::TestDecl(test) = decl {
            if !test.body.is_empty() {
                pruned += 1;
            }
            test.body.clear();
        }
    }
    pruned
}

fn stage_done(profile: bool, name: &str, t: Instant) {
    if profile {
        eprintln!("stage {name} {:.3}ms", t.elapsed().as_secs_f64() * 1000.0);
    }
}

/// Escape token text for the one-line-per-token dump format.
fn escape_bytes(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        match b {
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(b as char),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out
}

/// D-P3 prescan: for every `@import "<path>.sla"` in the root source,
/// resolve the path relative to the importing file's directory, read
/// the file, and collect the names of its top-level `struct` / `enum`
/// declarations (first-seen order). Returns (types, enums, notes).
/// One level only, matching what the Zig prescan contributes to the
/// importing file's literal/pattern gates; failures are noted, never
/// silently swallowed (contrast D-P11: the Zig prescan eats `catch {}`).
fn prescan_sla_imports(input: &Path, source: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut types: Vec<String> = Vec::new();
    let mut enums: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let base_dir = input.parent().unwrap_or_else(|| Path::new("."));
    for line in source.lines() {
        let Some(path) = import_path_of_line(line) else {
            continue;
        };
        if !path.ends_with(".sla") {
            continue;
        }
        let full = base_dir.join(&path);
        match std::fs::read_to_string(&full) {
            Ok(text) => {
                let (t, e) = scan_declared_type_names(&text);
                notes.push(format!(
                    "import {path}: +{} types +{} enums (from {})",
                    t.len(),
                    e.len(),
                    full.display()
                ));
                for name in t {
                    if !types.contains(&name) {
                        types.push(name);
                    }
                }
                for name in e {
                    if !enums.contains(&name) {
                        enums.push(name);
                    }
                }
            }
            Err(err) => {
                notes.push(format!("import {path}: UNREADABLE {} ({err})", full.display()));
            }
        }
    }
    if types.is_empty() && enums.is_empty() && notes.is_empty() {
        notes.push("no .sla imports: empty seed".to_string());
    }
    (types, enums, notes)
}

/// Extract the quoted path of an `@import "..."` line, if the line is one.
fn import_path_of_line(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line.strip_prefix("@import")?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Collect `struct Name` / `enum Name` declaration names from a source
/// text (line-anchored scan; an optional leading `pub` is allowed).
fn scan_declared_type_names(text: &str) -> (Vec<String>, Vec<String>) {
    let mut types = Vec::new();
    let mut enums = Vec::new();
    for line in text.lines() {
        let mut words = line.trim_start().split_whitespace();
        let mut kw = words.next().unwrap_or("");
        if kw == "pub" {
            kw = words.next().unwrap_or("");
        }
        let name = words.next().unwrap_or("");
        let valid = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_');
        if !valid {
            continue;
        }
        match kw {
            "struct" => types.push(name.to_string()),
            "enum" => enums.push(name.to_string()),
            _ => {}
        }
    }
    (types, enums)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sla_ast::{ImportDecl, Program, TestDecl};

    fn test_decl(name: &str, body: Vec<Node>) -> Node {
        Node::TestDecl(TestDecl {
            name: name.to_string(),
            is_ignored: true,
            should_panic: false,
            body,
        })
    }

    #[test]
    fn prune_empties_test_bodies_but_keeps_the_shell() {
        let mut root = Node::Program(Program {
            decls: vec![
                Node::ImportDecl(ImportDecl {
                    path: "sa_std/fmt.sai".to_string(),
                }),
                test_decl("full", vec![Node::Identifier("x".to_string())]),
                test_decl("already empty", Vec::new()),
            ],
        });
        assert_eq!(prune_test_bodies(&mut root), 1);
        let Node::Program(program) = &root else {
            panic!("root must stay a program");
        };
        // Non-test declarations are untouched.
        assert!(matches!(&program.decls[0], Node::ImportDecl(_)));
        for decl in &program.decls[1..] {
            let Node::TestDecl(test) = decl else {
                panic!("test decl must stay a test decl");
            };
            assert!(test.body.is_empty());
            assert!(test.is_ignored);
            assert!(!test.should_panic);
        }
        assert_eq!(program.decls.len(), 3);
        // Pruning is idempotent: a second pass clears nothing new.
        assert_eq!(prune_test_bodies(&mut root), 0);
    }

    #[test]
    fn prune_on_a_tree_without_tests_changes_nothing() {
        let mut root = Node::Program(Program {
            decls: vec![Node::ImportDecl(ImportDecl {
                path: "sa_std/fmt.sai".to_string(),
            })],
        });
        let before = format!("{root:?}");
        assert_eq!(prune_test_bodies(&mut root), 0);
        assert_eq!(format!("{root:?}"), before);
        // A non-program root is not a pipeline input; prune is a no-op.
        let mut leaf = Node::Identifier("x".to_string());
        assert_eq!(prune_test_bodies(&mut leaf), 0);
    }
}
