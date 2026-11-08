//! Corpus SAB reason probe (R-4 slice2 measurement harness).
//!
//! The CLI (`sla test --test-backend sab`) prints only the error
//! *name* of a failed SAB compile (`SAB Error: direct SLA-to-SAB
//! lowering failed: error.UnsupportedSabDirectFeature`); the reason
//! string the codegen attaches (divergence SAB-D11) is dropped at
//! the print site in `sla-cli/src/pipeline.rs`. This harness
//! reproduces the CLI's TestCodegen front end in-process and calls
//! `sla_codegen_sab::generate_from_program_with_flattener` with a
//! host flattener that mirrors `sla-cli/src/sa_flattener.rs`
//! byte-for-byte in its error strings, then prints the full
//! `Error` Display per corpus file as `PROBE` lines:
//!
//! ```text
//! PROBE <file> OK
//! PROBE <file> FRONTEND <stage>: <detail>
//! PROBE <file> ERR <Display of sla_codegen_sab::Error>
//! ```
//!
//! Classification rule: an `ERR` payload containing
//! `CLI-D4 sa-host flattener` is a host-generation artifact of the
//! pre-flatmat host `sa` (the venue host at sci d31faea6 refuses
//! `@target` hybrid std sources such as `sa_std/core/mem.sa` at the
//! emit-sab seam), NOT a codegen-sab coverage gap. Payloads without
//! that marker are genuine codegen-sab stops and cluster by their
//! `what [group ...]` reason text.
//!
//! Deliberate divergences from the CLI front end (registered):
//! - The two CLI-D5 prunes (`prune_tests_by_filter`,
//!   `prune_unreachable_test_function_decls_before_type_check`)
//!   live in sla-cli and are not importable from here; they are
//!   omitted. They only remove declarations, so a construct the
//!   probe reaches is also reachable by the CLI modulo pruning.
//! - Env knobs: `SA_EXE` (host sa path), `SLA_PROBE_CORPUS`
//!   (corpus dir), `SLA_PROBE_FILES` (space-separated file names
//!   to probe instead of the whole positive set), plus the usual
//!   `SLA_STD_DIR` / `SA_STD_DIR` consumed by `SurfaceRoots`.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use sla_ast::node::Node;
use sla_ast::Program;
use sla_checker::Checker;
use sla_codegen_sab::fragment_imports::StdSnippetFlattener;
use sla_codegen_sab::surface_load::SurfaceRoots;
use sla_codegen_sab::types::StdImportModule;
use sla_codegen_sab::{Error, Result};

// -- host flattener (mirror of sla-cli/src/sa_flattener.rs) -------------------

struct HostFlattener {
    sa_exe: String,
    import_cache: HashMap<String, StdImportModule>,
    counter: u64,
}

impl HostFlattener {
    fn flatten_source(&mut self, source: &str, tag: &str) -> Result<sla_codegen_sab::Module> {
        self.counter += 1;
        let dir = std::env::temp_dir().join(format!(
            "sla-probe-{}-{}",
            std::process::id(),
            self.counter
        ));
        self.flatten_source_in(&dir, source, tag)
    }

    fn flatten_source_in(
        &mut self,
        dir: &Path,
        source: &str,
        tag: &str,
    ) -> Result<sla_codegen_sab::Module> {
        std::fs::create_dir_all(dir).map_err(|e| {
            Error::UnsupportedSabDirectFeature(format!(
                "CLI-D4 sa-host flattener: cannot create temp dir {} for snippet `{tag}`: {e} [SAB-D31 host seam via `sa check --emit-sab`]",
                dir.display()
            ))
        })?;
        let src_path = dir.join("snippet.sa");
        let sab_path = dir.join("snippet.sab");
        std::fs::write(&src_path, source).map_err(|e| {
            Error::UnsupportedSabDirectFeature(format!(
                "CLI-D4 sa-host flattener: cannot write snippet `{tag}` to {}: {e} [SAB-D31 host seam]",
                src_path.display()
            ))
        })?;
        let output = std::process::Command::new(&self.sa_exe)
            .current_dir(dir)
            .arg("check")
            .arg(&src_path)
            .arg("--emit-sab")
            .arg(&sab_path)
            .output()
            .map_err(|e| {
                Error::UnsupportedSabDirectFeature(format!(
                    "CLI-D4 sa-host flattener: cannot run host `{} check` for snippet `{tag}`: {e} [SAB-D31 host seam; resolved sa executable is used verbatim, no fallback]",
                    self.sa_exe
                ))
            })?;
        if !output.status.success() {
            let host_words = String::from_utf8_lossy(&output.stderr);
            let host_words = host_words.trim();
            let host_words = if host_words.is_empty() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                host_words.to_string()
            };
            return Err(Error::UnsupportedSabDirectFeature(format!(
                "CLI-D4 sa-host flattener: host `sa check --emit-sab` refused snippet `{tag}` (exit {}): {} [SAB-D31 host seam; the host verified encode is the only encode the sa CLI offers — see CLI-D4 residual in docs/divergences/cli-commands.md]",
                output.status, host_words
            )));
        }
        let bytes = std::fs::read(&sab_path).map_err(|e| {
            Error::UnsupportedSabDirectFeature(format!(
                "CLI-D4 sa-host flattener: host reported success for snippet `{tag}` but its SAB product {} is unreadable: {e} [SAB-D31 host seam]",
                sab_path.display()
            ))
        })?;
        sla_codegen_sab::decode_module(&bytes).map_err(|e| {
            Error::UnsupportedSabDirectFeature(format!(
                "CLI-D4 sa-host flattener: SAB product for snippet `{tag}` does not decode: {e} [SAB-D31 host seam]"
            ))
        })
    }
}

impl StdSnippetFlattener for HostFlattener {
    fn flatten_import_module(&mut self, import_path: &str) -> Result<StdImportModule> {
        if let Some(module) = self.import_cache.get(import_path) {
            return Ok(module.clone());
        }
        let source = format!("@import \"{import_path}\"\n");
        let module = self.flatten_source(&source, import_path)?;
        let view = StdImportModule {
            import_path: import_path.to_string(),
            symbols: module.symbols,
            function_sigs: module.function_sigs,
            const_decls: module.const_decls,
            instructions: module.instructions,
        };
        self.import_cache.insert(import_path.to_string(), view.clone());
        Ok(view)
    }

    fn flatten_and_decode(
        &mut self,
        source: &str,
    ) -> Result<sla_codegen_sab::Module> {
        self.flatten_source(source, "macro-fragment snippet")
    }
}

// -- CheckInfo adapter (same wiring shape as sla-cli / sla-oracle) ------------

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

// -- seed prescan (copied from sla-cli pipeline.rs, D-P3 line scan) -----------

fn import_path_of_line(line: &str) -> Option<String> {
    let line = line.trim();
    let rest = line.strip_prefix("@import")?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

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

// -- per-file probe --------------------------------------------------------------

fn probe_file(path: &Path, sa_exe: &str) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return format!("PROBE {name} FRONTEND read: {e}"),
    };
    let (seed_types, seed_enums) = prescan_sla_imports(path, &content);
    let options = sla_parser::ParserOptions {
        parse_test_bodies: true,
        ..Default::default()
    };
    let mut parser = sla_parser::Parser::with_options(&content, options);
    let type_refs: Vec<&str> = seed_types.iter().map(String::as_str).collect();
    let enum_refs: Vec<&str> = seed_enums.iter().map(String::as_str).collect();
    parser.seed_known_type_names(&type_refs, &enum_refs);
    let mut root = match parser.parse_program() {
        Ok(root) => root,
        Err(err) => return format!("PROBE {name} FRONTEND parse: {err:?}"),
    };
    // TestCodegen .sla import BODY expansion (R-2 mechanism).
    let program: &Program = {
        let Node::Program(parsed) = &root else {
            return format!("PROBE {name} FRONTEND shape: root is not a program");
        };
        let exp_options = sla_plugin::sla_expand::SlaImportExpansionOptions {
            include_all_imported_decls: true,
            imported_bodies_decl_only: false,
            ..Default::default()
        };
        match sla_plugin::sla_expand::expand_sla_imports(parsed, path, &exp_options) {
            Ok(expanded) => {
                root = Node::Program(expanded.program);
            }
            Err(err) => return format!("PROBE {name} FRONTEND expand: {err}"),
        }
        let Node::Program(program) = &root else {
            return format!("PROBE {name} FRONTEND shape: root is not a program");
        };
        program
    };
    let mut checker = Checker::new();
    let import_paths: Vec<String> = program
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Node::ImportDecl(import) => Some(import.path.clone()),
            _ => None,
        })
        .collect();
    let report = match sla_parser::contract_load::load_imported_contracts(&import_paths, path) {
        Ok(report) => report,
        Err(err) => return format!("PROBE {name} FRONTEND contracts: {err}"),
    };
    for func in &report.externs {
        let sig = sla_checker::check_overload3::extern_sig_from_parts(
            &func
                .params
                .iter()
                .map(|pp| (pp.name.clone(), pp.ty.clone(), pp.is_borrow, pp.is_move))
                .collect::<Vec<_>>(),
            &func.ret_ty,
            func.return_fallible,
            func.is_variadic,
        );
        if let Err(diag) = checker.register_extern_contract(&func.name, sig) {
            return format!(
                "PROBE {name} FRONTEND register-extern: {} (error.{})",
                diag.message,
                diag.kind.name()
            );
        }
    }
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
    if let Err(diag) = checker.check_program(program) {
        return format!(
            "PROBE {name} FRONTEND check: {} (error.{})",
            diag.message,
            diag.kind.name()
        );
    }
    let info = CheckerInfo(&checker);
    let flattener: Box<dyn StdSnippetFlattener> = Box::new(HostFlattener {
        sa_exe: sa_exe.to_string(),
        import_cache: HashMap::new(),
        counter: 0,
    });
    match sla_codegen_sab::generate_from_program_with_flattener(
        &root,
        &info,
        SurfaceRoots::from_env(),
        flattener,
    ) {
        Ok(_) => format!("PROBE {name} OK"),
        Err(err) => {
            let text = err.to_string().replace(['\n', '\r'], " ");
            format!("PROBE {name} ERR {text}")
        }
    }
}

#[test]
fn corpus_sab_probe() {
    let sa_exe = std::env::var("SA_EXE").unwrap_or_else(|_| "sa".to_string());
    let corpus = std::env::var("SLA_PROBE_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/tests")
        });
    let files: Vec<PathBuf> = match std::env::var("SLA_PROBE_FILES") {
        Ok(list) => list
            .split_whitespace()
            .map(|n| corpus.join(n))
            .collect(),
        Err(_) => {
            let mut names: Vec<PathBuf> = std::fs::read_dir(&corpus)
                .expect("corpus dir readable")
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    let n = p.file_name().unwrap_or_default().to_string_lossy();
                    n.ends_with(".sla")
                        && !n.starts_with("test_error_")
                        && !n.starts_with("neg_")
                })
                .collect();
            names.sort();
            names
        }
    };
    // Full-run output goes to a file when SLA_PROBE_OUT is set:
    // routing 567 long lines through the test harness's captured
    // stdout lost lines in calibration (only 2 of 3 arrived via a
    // pipe), so batch runs write directly.
    let mut out: Box<dyn std::io::Write> = match std::env::var("SLA_PROBE_OUT") {
        Ok(path) => Box::new(
            std::fs::File::create(&path).unwrap_or_else(|e| panic!("create {path}: {e}")),
        ),
        Err(_) => Box::new(std::io::stdout()),
    };
    for file in &files {
        let line = probe_file(file, &sa_exe);
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}
