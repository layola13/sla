//! CLI-D4 provider: a real `StdSnippetFlattener` backed by the
//! host `sa` executable — the standalone binary's form of the
//! SAB-D31 sci boundary.
//!
//! In the Zig topology `flattenStdSnippet` is host machinery
//! (`sci_bridge`): SA-text flattener + verifier + SAB encode,
//! reached in-process. The standalone `sla` binary has no
//! in-process host, so this provider reaches the same machinery
//! through the installed `sa` command line — the architecture
//! seam phase 4 already ruled on for the delegated build tails
//! (see delegate.rs). The exact host entry is
//! `sa check <snippet.sa> --emit-sab <out.sab>`: sci's
//! `executeCheck` flattens, verifies, and encodes from the flat
//! itself (`encodeSabFromFlat`, which re-verifies), so the bytes
//! it writes are precisely the flatten+verify+encode chain the
//! Zig code calls, and this provider decodes them with the
//! workspace's own SAB decoder (`sla_codegen_sab::decode_module`).
//!
//! Shape notes, by trait method:
//!   * `flatten_import_module(path)` builds the exact Zig source
//!     (`@import "{path}"\n`, sab_codegen.zig `cachedStdImportModule`)
//!     and reassembles the decoded module into the
//!     `StdImportModule` view. Results are cached in-process by
//!     import path (the fragment machine caches again above this;
//!     both caches are lookup-only maps, never iterated).
//!   * `flatten_and_decode(source)` runs the caller's snippet
//!     verbatim. One honest asymmetry with the Zig side: Zig's
//!     template/fragment path encodes with
//!     `encodeSabFromFlatUnchecked`, while `sa check --emit-sab`
//!     only offers the *verified* encode — a snippet whose
//!     placeholder registers cannot verify standalone (the
//!     `__sla_macro_arg_{n}` template form: an output placeholder
//!     is live at the snippet's own `return`) is refused by the
//!     host verifier and this provider surfaces that refusal
//!     loudly, with the host's trap text. Registered in
//!     docs/divergences/cli-commands.md (CLI-D4 residual).
//!
//! Failure discipline: no silent fallback anywhere. A missing
//! `sa`, a failed spawn, a non-zero host exit, an unreadable or
//! undecodable SAB product — each returns
//! `Error::UnsupportedSabDirectFeature` naming the seam, the
//! snippet's identity, and the host's own words, and the SAB
//! pipeline aborts. Temp files live in a per-call unique
//! directory under the system temp dir and are removed after
//! the call (removal is best-effort housekeeping only; a failed
//! removal never masks the real result).

use sla_codegen_sab::error::{Error, Result};
use sla_codegen_sab::types::StdImportModule;
use sla_codegen_sab::{decode_module, Module, StdSnippetFlattener};
use std::collections::HashMap;
use std::path::PathBuf;

/// The Zig `cachedStdImportModule` snippet source for one
/// import path: a lone `@import` line.
pub fn import_snippet_source(import_path: &str) -> String {
    format!("@import \"{import_path}\"\n")
}

pub struct SaHostFlattener {
    sa_exe: String,
    import_cache: HashMap<String, StdImportModule>,
    call_counter: u64,
}

impl SaHostFlattener {
    pub fn new(sa_exe: String) -> Self {
        SaHostFlattener {
            sa_exe,
            import_cache: HashMap::new(),
            call_counter: 0,
        }
    }

    /// Flatten + verify + encode `source` through the host and
    /// decode the resulting SAB. `tag` identifies the snippet in
    /// diagnostics (the import path, or a fixed label).
    fn flatten_source(&mut self, source: &str, tag: &str) -> Result<Module> {
        self.call_counter += 1;
        // The directory must be unique per CALL across every
        // provider instance in the process, not just per
        // provider: a per-instance counter restarts at 1, and
        // two providers alive at once (the unit tests run
        // providers in parallel threads) would share a directory
        // and delete each other's snippet mid-check — the host
        // then dies with a bare FileNotFound. A process-wide
        // atomic sequence keeps names unique; the sequence is
        // only a name, never an emission input.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "sla-d4-flat-{}-{}",
            std::process::id(),
            seq
        ));
        let outcome = self.flatten_source_in(&dir, source, tag);
        // Housekeeping only: the compile result (or its error)
        // is already decided above and is never masked by this.
        let _ = std::fs::remove_dir_all(&dir);
        outcome
    }

    fn flatten_source_in(&mut self, dir: &PathBuf, source: &str, tag: &str) -> Result<Module> {
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
        // The child's cwd is pinned to the snippet's own temp
        // dir (which exists for the whole call): the host
        // resolves its project context from the cwd, and a
        // caller's cwd is not guaranteed to outlive the call
        // (the sla-cli freshness tests chdir into temp dirs
        // that are deleted on drop — a host spawned then died
        // with FileNotFound before ever reading the snippet).
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
        decode_module(&bytes).map_err(|e| {
            Error::UnsupportedSabDirectFeature(format!(
                "CLI-D4 sa-host flattener: SAB product for snippet `{tag}` does not decode: {e} [SAB-D31 host seam]"
            ))
        })
    }
}

impl StdSnippetFlattener for SaHostFlattener {
    fn flatten_import_module(&mut self, import_path: &str) -> Result<StdImportModule> {
        if let Some(module) = self.import_cache.get(import_path) {
            return Ok(module.clone());
        }
        let source = import_snippet_source(import_path);
        let module = self.flatten_source(&source, import_path)?;
        let view = StdImportModule {
            import_path: import_path.to_string(),
            symbols: module.symbols,
            function_sigs: module.function_sigs,
            const_decls: module.const_decls,
            instructions: module.instructions,
        };
        self.import_cache
            .insert(import_path.to_string(), view.clone());
        Ok(view)
    }

    fn flatten_and_decode(&mut self, source: &str) -> Result<Module> {
        self.flatten_source(source, "macro-fragment snippet")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_snippet_source_is_the_zig_lone_import_line() {
        assert_eq!(
            import_snippet_source("sa_std/io/print.sai"),
            "@import \"sa_std/io/print.sai\"\n"
        );
    }

    #[test]
    fn missing_host_is_a_loud_error_not_a_fallback() {
        let mut provider = SaHostFlattener::new("/nonexistent/sa-d4-test".to_string());
        match provider.flatten_and_decode("@x() -> void:\nL_ENTRY:\n    return\n") {
            Err(Error::UnsupportedSabDirectFeature(msg)) => {
                assert!(msg.contains("CLI-D4 sa-host flattener"), "{msg}");
                assert!(msg.contains("SAB-D31"), "{msg}");
            }
            other => panic!("expected loud host-seam error, got {other:?}"),
        }
        match provider.flatten_import_module("sa_std/io/print.sai") {
            Err(Error::UnsupportedSabDirectFeature(msg)) => {
                assert!(msg.contains("sa_std/io/print.sai"), "{msg}");
            }
            other => panic!("expected loud host-seam error, got {other:?}"),
        }
    }

    /// Live-host round trip, run where a real `sa` is resolvable
    /// (the box): the print contract's lone extern must come back
    /// as a decoded import-module view. Skipped loudly in the
    /// test log — never silently — when no host exists.
    #[test]
    fn live_host_flattens_print_contract() {
        let sa = crate::delegate::resolve_sa_executable();
        if !std::path::Path::new(&sa).exists() && sa == "sa" {
            eprintln!("live_host_flattens_print_contract: no `sa` on PATH, host round trip not exercised");
            return;
        }
        let mut provider = SaHostFlattener::new(sa);
        let module = provider
            .flatten_import_module("sa_std/io/print.sai")
            .expect("print contract flattens through the host");
        assert_eq!(module.import_path, "sa_std/io/print.sai");
        assert!(
            module.function_sigs.iter().any(|s| s.name == "sa_print_bytes"),
            "extern sig present: {:?}",
            module.function_sigs.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        // Second call is served by the provider cache and agrees.
        let again = provider
            .flatten_import_module("sa_std/io/print.sai")
            .expect("cached import module");
        assert_eq!(again, module);
    }
}
