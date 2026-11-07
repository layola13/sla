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
//! (see delegate.rs). The two trait methods take two different
//! host routes, because the Zig side itself runs two different
//! chains:
//!
//!   * `flatten_import_module(path)` is Zig's
//!     `cachedStdImportModule` chain — flatten + *verified*
//!     encode + sig clone of a lone `@import` line. The host
//!     entry with exactly that shape is
//!     `sa check <snippet.sa> --emit-sab <out.sab>`: sci's
//!     `executeCheck` flattens, verifies, and encodes from the
//!     flat itself (`encodeSabFromFlat`, which re-verifies), so
//!     the bytes it writes are precisely the chain the Zig code
//!     calls, and this provider decodes them with the
//!     workspace's own SAB decoder (`sla_codegen_sab::decode_module`).
//!   * `flatten_and_decode(source)` is Zig's template/fragment
//!     chain — `flattenStdSnippet` +
//!     `encodeSabFromFlatUnchecked` + `sab.decodeModule`. The
//!     payload is encoded *unchecked* because a fragment is not
//!     a program: its placeholder registers (`__sla_macro_arg_{n}`)
//!     are owned by the assembling compiler, so the verified
//!     whole-program encoder refuses it (inputs trap as
//!     UnknownRegister, outputs as MemoryLeak at the wrapper's
//!     exit). The host route with exactly that shape is
//!     `sa encode-fragment` (sci `sab_fragment.zig`, fragment
//!     format 1.0): it validates a `#fragment` contract header
//!     against the flattened wrapper body, encodes the payload
//!     with the *same* unchecked encoder implementation the
//!     plugin uses in-process, and wraps it in the versioned
//!     SAFRAG container. This provider synthesizes the contract
//!     header for the machine-built snippet, refines it against
//!     the host's own contract validation (each violation names
//!     the register and the set it belongs to; the provider
//!     transcribes the host's recomputation, it never guesses a
//!     set the host has not stated), unwraps the container, and
//!     decodes the payload module. A bare SAFRAG is rejected by
//!     every ordinary SAB entry point on the host side
//!     (SA-CLI-022 SabFragmentNotAProgram); here it never leaves
//!     the provider — only the decoded payload module does.
//!
//! Failure discipline: no silent fallback anywhere. A missing
//! `sa`, a failed spawn, a non-zero host exit, a contract that
//! does not converge, an undecodable container or payload —
//! each returns `Error::UnsupportedSabDirectFeature` naming the
//! seam, the snippet's identity, and the host's own words, and
//! the SAB pipeline aborts. Temp files live in a per-call
//! unique directory under the system temp dir and are removed
//! after the call (removal is best-effort housekeeping only; a
//! failed removal never masks the real result).

use sla_codegen_sab::error::{Error, Result};
use sla_codegen_sab::types::StdImportModule;
use sla_codegen_sab::{decode_module, Module, StdSnippetFlattener};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

/// The Zig `cachedStdImportModule` snippet source for one
/// import path: a lone `@import` line.
pub fn import_snippet_source(import_path: &str) -> String {
    format!("@import \"{import_path}\"\n")
}

/// Caller attribution written into every fragment contract
/// header (the host records it in the container).
fn fragment_caller() -> String {
    format!("sla-rust {}", env!("CARGO_PKG_VERSION"))
}

/// Run `body` in a per-call unique temp directory. The
/// directory must be unique per CALL across every provider
/// instance in the process, not just per provider: a
/// per-instance counter restarts at 1, and two providers alive
/// at once (the unit tests run providers in parallel threads)
/// would share a directory and delete each other's snippet
/// mid-check — the host then dies with a bare FileNotFound. A
/// process-wide atomic sequence keeps names unique; the
/// sequence is only a name, never an emission input.
fn with_temp_dir<T>(body: impl FnOnce(&PathBuf) -> Result<T>) -> Result<T> {
    static SEQ: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "sla-d4-flat-{}-{}",
        std::process::id(),
        seq
    ));
    let outcome = body(&dir);
    // Housekeeping only: the result (or its error) is already
    // decided above and is never masked by this.
    let _ = std::fs::remove_dir_all(&dir);
    outcome
}

pub struct SaHostFlattener {
    sa_exe: String,
    import_cache: HashMap<String, StdImportModule>,
}

impl SaHostFlattener {
    pub fn new(sa_exe: String) -> Self {
        SaHostFlattener {
            sa_exe,
            import_cache: HashMap::new(),
        }
    }

    /// Flatten + verify + encode `source` through the host's
    /// verified route (`sa check --emit-sab`) and decode the
    /// resulting SAB. `tag` identifies the snippet in
    /// diagnostics (the import path, or a fixed label).
    fn flatten_source(&mut self, source: &str, tag: &str) -> Result<Module> {
        with_temp_dir(|dir| self.flatten_source_in(dir, source, tag))
    }

    fn flatten_source_in(
        &mut self,
        dir: &PathBuf,
        source: &str,
        tag: &str,
    ) -> Result<Module> {
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
            let host_words = host_output_words(&output);
            return Err(Error::UnsupportedSabDirectFeature(format!(
                "CLI-D4 sa-host flattener: host `sa check --emit-sab` refused snippet `{tag}` (exit {}): {} [SAB-D31 host seam; the verified encode is the only encode the check route offers]",
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

    /// Flatten + contract-encode `source` through the host's
    /// fragment route (`sa encode-fragment`), unwrap the SAFRAG
    /// container, and decode the payload module.
    fn flatten_fragment(&mut self, source: &str, tag: &str) -> Result<Module> {
        let shape = FragmentShape::from_source(source).ok_or_else(|| {
            Error::UnsupportedSabDirectFeature(format!(
                "CLI-D4 sa-host flattener: fragment snippet `{tag}` declares no wrapper function, so no #fragment contract can be synthesized for it [SAB-D31 host seam via `sa encode-fragment`]"
            ))
        })?;
        with_temp_dir(|dir| self.flatten_fragment_in(dir, source, tag, &shape))
    }

    fn flatten_fragment_in(
        &mut self,
        dir: &PathBuf,
        source: &str,
        tag: &str,
        shape: &FragmentShape,
    ) -> Result<Module> {
        std::fs::create_dir_all(dir).map_err(|e| {
            Error::UnsupportedSabDirectFeature(format!(
                "CLI-D4 sa-host flattener: cannot create temp dir {} for fragment `{tag}`: {e} [SAB-D31 host seam via `sa encode-fragment`]",
                dir.display()
            ))
        })?;
        let src_path = dir.join("snippet.sa");
        let frag_path = dir.join("snippet.safrag");
        let mut contract = FragmentContract::seeded(shape);
        // The host recomputes the exact register sets from the
        // flattened body and names one mismatch per run, in
        // sorted order; each round transcribes one named fact.
        // The register universe of one fragment is small (the
        // goldens need at most a handful of rounds); the cap is
        // a loud stop, never a silent give-up into a fallback.
        const MAX_ROUNDS: usize = 128;
        let mut last_violation: Option<(String, Option<String>)> = None;
        for _round in 0..MAX_ROUNDS {
            let full_source = contract.header() + source;
            std::fs::write(&src_path, &full_source).map_err(|e| {
                Error::UnsupportedSabDirectFeature(format!(
                    "CLI-D4 sa-host flattener: cannot write fragment `{tag}` to {}: {e} [SAB-D31 host seam]",
                    src_path.display()
                ))
            })?;
            let output = std::process::Command::new(&self.sa_exe)
                .current_dir(dir)
                .arg("encode-fragment")
                .arg(&src_path)
                .arg("-o")
                .arg(&frag_path)
                .arg("--json")
                .output()
                .map_err(|e| {
                    Error::UnsupportedSabDirectFeature(format!(
                        "CLI-D4 sa-host flattener: cannot run host `{} encode-fragment` for fragment `{tag}`: {e} [SAB-D31 host seam; resolved sa executable is used verbatim, no fallback]",
                        self.sa_exe
                    ))
                })?;
            if output.status.success() {
                let bytes = std::fs::read(&frag_path).map_err(|e| {
                    Error::UnsupportedSabDirectFeature(format!(
                        "CLI-D4 sa-host flattener: host reported success for fragment `{tag}` but its SAFRAG product {} is unreadable: {e} [SAB-D31 host seam]",
                        frag_path.display()
                    ))
                })?;
                let payload = decode_safrag_payload(&bytes).map_err(|e| {
                    Error::UnsupportedSabDirectFeature(format!(
                        "CLI-D4 sa-host flattener: SAFRAG product for fragment `{tag}` does not unwrap: {e} [SAB-D31 host seam]"
                    ))
                })?;
                return decode_module(payload).map_err(|e| {
                    Error::UnsupportedSabDirectFeature(format!(
                        "CLI-D4 sa-host flattener: SAFRAG payload for fragment `{tag}` does not decode as a SAB module: {e} [SAB-D31 host seam]"
                    ))
                });
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            match parse_fragment_violation(&stdout) {
                Some((kind, register)) => {
                    let key = (kind.clone(), register.clone());
                    if last_violation.as_ref() == Some(&key)
                        || !contract.apply_violation(&kind, register.as_deref())
                    {
                        return Err(Error::UnsupportedSabDirectFeature(format!(
                            "CLI-D4 sa-host flattener: host `sa encode-fragment` contract validation for fragment `{tag}` made no progress at violation `{kind}` (register: {}): {} [SAB-D31 host seam]",
                            register.as_deref().unwrap_or("-"),
                            host_output_words(&output)
                        )));
                    }
                    last_violation = Some(key);
                }
                None => {
                    return Err(Error::UnsupportedSabDirectFeature(format!(
                        "CLI-D4 sa-host flattener: host `sa encode-fragment` refused fragment `{tag}` (exit {}): {} [SAB-D31 host seam; a fragment is not a program and no other host route encodes it]",
                        output.status,
                        host_output_words(&output)
                    )));
                }
            }
        }
        Err(Error::UnsupportedSabDirectFeature(format!(
            "CLI-D4 sa-host flattener: host `sa encode-fragment` contract validation for fragment `{tag}` did not converge in {MAX_ROUNDS} rounds [SAB-D31 host seam]"
        )))
    }
}

/// The host's own words for a failed run: stderr if it carries
/// anything, stdout otherwise (the previous form's rule).
fn host_output_words(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        return stderr.to_string();
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// The fragment-relevant shape of a machine-built snippet: the
/// wrapper function it declares and the placeholder registers
/// it references, in first-appearance order.
struct FragmentShape {
    function_name: String,
    placeholders: Vec<String>,
}

impl FragmentShape {
    fn from_source(source: &str) -> Option<FragmentShape> {
        let mut function_name: Option<String> = None;
        for line in source.lines() {
            let line = line.trim();
            if !line.starts_with('@') {
                continue;
            }
            let rest = &line[1..];
            let word_end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let word = &rest[..word_end];
            match word {
                // Environment lines, not the wrapper declaration.
                "import" | "const" | "extern" => continue,
                _ => {
                    function_name = Some(word.to_string());
                    break;
                }
            }
        }
        let function_name = function_name?;
        let mut placeholders: Vec<String> = Vec::new();
        let bytes = source.as_bytes();
        let needle = b"__sla_macro_arg_";
        let mut i = 0;
        while i + needle.len() < bytes.len() {
            if &bytes[i..i + needle.len()] == needle {
                let mut j = i + needle.len();
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > i + needle.len() {
                    let name = &source[i..j];
                    if !placeholders.iter().any(|p| p == name) {
                        placeholders.push(name.to_string());
                    }
                }
                i = j;
            } else {
                i += 1;
            }
        }
        Some(FragmentShape {
            function_name,
            placeholders,
        })
    }
}

/// A `#fragment` contract header under refinement. The sets are
/// kept in sorted containers so the composed header (and thus
/// the container the host derives from it) is deterministic —
/// no hash-order ever reaches an emitted byte.
#[derive(Default)]
struct FragmentContract {
    function_name: String,
    reads: BTreeSet<String>,
    owns: BTreeSet<String>,
    returns: BTreeSet<String>,
}

impl FragmentContract {
    /// Seed from the golden shape of the std macro classes
    /// (sci `tests/fragment_smoke.zig`): the first placeholder
    /// is the macro's out register (defined by the expansion,
    /// handed back to the assembler), the rest are inputs.
    /// The seed only sets the starting point — the host's
    /// validation corrects it register by register, and any
    /// seed that already matches costs exactly one host run.
    fn seeded(shape: &FragmentShape) -> FragmentContract {
        let mut contract = FragmentContract {
            function_name: shape.function_name.clone(),
            ..Default::default()
        };
        for (idx, name) in shape.placeholders.iter().enumerate() {
            if idx == 0 {
                contract.owns.insert(name.clone());
                contract.returns.insert(name.clone());
            } else {
                contract.reads.insert(name.clone());
            }
        }
        contract
    }

    fn header(&self) -> String {
        fn list(set: &BTreeSet<String>) -> String {
            set.iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
        format!(
            "#fragment 1\n#fragment-function: {}\n#fragment-reads: {}\n#fragment-owns: {}\n#fragment-returns: {}\n#fragment-caller: {}\n",
            self.function_name,
            list(&self.reads),
            list(&self.owns),
            list(&self.returns),
            fragment_caller()
        )
    }

    /// Transcribe one host-stated contract fact. Returns false
    /// when the violation kind is not a set fact (program
    /// surface, entry mismatch — the snippet is not
    /// fragment-shaped; the caller stops loudly) or when the
    /// named fact is already reflected (no progress).
    fn apply_violation(&mut self, kind: &str, register: Option<&str>) -> bool {
        let register = match register {
            Some(name) => name,
            None => return false,
        };
        match kind {
            "undeclared_external" => self.reads.insert(register.to_string()),
            "unused_read" => self.reads.remove(register),
            "undeclared_own" => self.owns.insert(register.to_string()),
            "unused_own" => self.owns.remove(register),
            "foreign_release" => self.reads.insert(register.to_string()),
            "release_of_returned" => self.returns.remove(register),
            "return_outside_contract" => {
                if self.returns.contains(register)
                    && !self.reads.contains(register)
                    && !self.owns.contains(register)
                {
                    self.returns.remove(register)
                } else {
                    self.returns.insert(register.to_string())
                }
            }
            _ => false,
        }
    }
}

/// Extract a `FragmentContractViolation` (kind, register) from
/// the host's `--json` report. The report shapes are the host's
/// own (`executeEncodeFragment`): an error object whose `name`
/// is `FragmentContractViolation`, with `kind` and `register`
/// (a string or null) beside it. Anything else — header parse
/// errors, flatten traps, plain text — is not a violation and
/// returns None; the caller then fails loudly with the raw
/// host words.
fn parse_fragment_violation(stdout: &str) -> Option<(String, Option<String>)> {
    if !stdout.contains("\"FragmentContractViolation\"") {
        return None;
    }
    let kind = json_string_field(stdout, "\"kind\"")?;
    let register = if stdout.contains("\"register\":null") {
        None
    } else {
        json_string_field(stdout, "\"register\"")
    };
    Some((kind, register))
}

/// Read a `"key":"value"` string field from a flat JSON text.
/// Values read this way are host tags and register names —
/// identifier-shaped, never carrying escapes.
fn json_string_field(text: &str, key: &str) -> Option<String> {
    let at = text.find(key)?;
    let rest = &text[at + key.len()..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// The SAFRAG container magic (sci `sab.fragment_magic`).
const SAFRAG_MAGIC: &[u8] = b"SAFRAG\0";
/// Fragment container format this provider speaks
/// (sci `sab_fragment.format_major/minor`); frontends pin the
/// version and a mismatch is a loud error.
const SAFRAG_FORMAT: (u8, u8) = (1, 0);

/// Unwrap a SAFRAG 1.0 container and return its payload slice
/// (the unchecked-encoded SAB module). The layout mirrors
/// sci `sab_fragment.encodeContainer` field for field; every
/// structural problem is a named error, never a reinterpretation.
fn decode_safrag_payload(bytes: &[u8]) -> std::result::Result<&[u8], String> {
    if !bytes.starts_with(SAFRAG_MAGIC) {
        return Err("InvalidFragmentMagic: product does not start with the SAFRAG magic".into());
    }
    let mut cursor = SafragCursor {
        bytes,
        index: SAFRAG_MAGIC.len(),
    };
    let fmt_major = cursor.read_byte()?;
    let fmt_minor = cursor.read_byte()?;
    if (fmt_major, fmt_minor) != SAFRAG_FORMAT {
        return Err(format!(
            "UnsupportedFragmentVersion: container fragment format is {fmt_major}.{fmt_minor}, this provider speaks {}.{}",
            SAFRAG_FORMAT.0, SAFRAG_FORMAT.1
        ));
    }
    // SAB version of the payload encoding: the payload carries
    // its own version and `decode_module` enforces it; here the
    // pair is read for the record and a zero (truncated/absent)
    // value is structural damage.
    let sab_major = cursor.read_byte()?;
    let _sab_minor = cursor.read_byte()?;
    if sab_major == 0 {
        return Err("TruncatedFragment: container carries no SAB version".into());
    }
    cursor.read_slice(32)?; // source sha256 (attribution record)
    cursor.read_name()?; // caller
    cursor.read_name()?; // function name
    cursor.read_name_list()?; // reads
    cursor.read_name_list()?; // owns
    cursor.read_name_list()?; // returns
    let payload_len = cursor.read_uleb()?;
    cursor.read_slice(payload_len)
}

struct SafragCursor<'a> {
    bytes: &'a [u8],
    index: usize,
}

impl<'a> SafragCursor<'a> {
    fn read_byte(&mut self) -> std::result::Result<u8, String> {
        if self.index >= self.bytes.len() {
            return Err("TruncatedFragment: container ends mid-header".into());
        }
        let byte = self.bytes[self.index];
        self.index += 1;
        Ok(byte)
    }

    fn read_slice(
        &mut self,
        len: usize,
    ) -> std::result::Result<&'a [u8], String> {
        if len > self.bytes.len() - self.index {
            return Err("TruncatedFragment: container ends mid-field".into());
        }
        let start = self.index;
        self.index += len;
        Ok(&self.bytes[start..self.index])
    }

    fn read_uleb(&mut self) -> std::result::Result<usize, String> {
        let mut result: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            let byte = self.read_byte()?;
            result |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return usize::try_from(result)
                    .map_err(|_| "TruncatedFragment: length overflows".to_string());
            }
            if shift >= 63 {
                return Err("TruncatedFragment: uleb128 run too long".into());
            }
            shift += 7;
        }
    }

    fn read_name(&mut self) -> std::result::Result<&'a [u8], String> {
        let len = self.read_uleb()?;
        self.read_slice(len)
    }

    fn read_name_list(&mut self) -> std::result::Result<(), String> {
        let count = self.read_uleb()?;
        for _ in 0..count {
            self.read_name()?;
        }
        Ok(())
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
        self.flatten_fragment(source, "macro-fragment snippet")
    }
}

#[cfg(test)]
mod tests;
