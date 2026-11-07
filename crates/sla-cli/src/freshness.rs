//! Freshness gate — mirror of `plugin_freshness.zig`'s LOCAL gate
//! (venue head 2cc12ea8). The Zig file has two storage paths: a
//! host-broker path (used when a host Context with a broker is
//! present) and a local on-disk gate; its own comment states the
//! broker is "null for the standalone sla binary, where the local
//! gate below is the authority" — so this binary mirrors the local
//! gate exactly:
//!
//!   * key = Wyhash over (compiler stamp, root file, options
//!     digest), stored under `.sla-cache/fresh/{stem}-{key:x}.bin`
//!     (artifact) + `.json` (manifest {version, kind, deps} with a
//!     per-dep {path, len, Wyhash(0, bytes)} closure fingerprint);
//!   * a load hits only when every recorded dep still hashes to
//!     the recorded value; any doubt is a miss (full compile);
//!   * `SLA_NO_FRESHNESS_CACHE` set to anything but "" / "0" /
//!     "false" disables both load and store.
//!
//! Compiler stamp (cache_version.zig `mixInto`): the key mixes
//! `key_version` (1) and a toolchain version string. The doctrine
//! in that file is that the stamp names THE COMPILER THAT WROTE
//! the entry, so entries roll when the compiler changes. This
//! binary therefore stamps with its own version
//! (`CARGO_PKG_VERSION`), not the Zig plugin's "0.1.8": a Rust
//! entry and a Zig entry for the same inputs live under different
//! keys and can never serve each other's artifacts. Gate
//! semantics, file layout, and manifest format are byte-mirrored.

use crate::wyhash::{wyhash, WyhashStream};

pub const KEY_VERSION: u32 = 1;
const MANIFEST_VERSION: u32 = 1;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DEP_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FreshKind {
    Sab,
    SaText,
}

impl FreshKind {
    pub fn tag(self) -> &'static str {
        match self {
            FreshKind::Sab => "sab",
            FreshKind::SaText => "sa",
        }
    }
}

/// The subset of SlaCompileOptions the digest reads
/// (plugin_compile_options.zig field order is NOT the digest
/// order; the digest order is plugin_freshness.zig's).
#[derive(Clone, Default)]
pub struct CompileOptions {
    pub test_filter: Option<String>,
    pub prune_for_test_codegen: bool,
    pub load_reachable_imported_bodies_from_registry: bool,
    pub goroutine_coop: bool,
    pub prune_for_entry_function: Option<String>,
}

impl CompileOptions {
    /// plugin_compile_options.zig `defaultSlaCompileOptions`.
    pub fn defaults() -> CompileOptions {
        CompileOptions {
            load_reachable_imported_bodies_from_registry: true,
            ..Default::default()
        }
    }
    /// The build-exe family options (`buildExeCompileOptions` in
    /// plugin_commands.zig) — identical field values to defaults.
    pub fn build_exe() -> CompileOptions {
        CompileOptions::defaults()
    }
    /// build-wasm adds the cooperative goroutine lowering.
    pub fn build_wasm() -> CompileOptions {
        let mut o = CompileOptions::defaults();
        o.goroutine_coop = true;
        o
    }
    /// Test-input compiles (plugin_compile.zig
    /// compileSlaSabTestInput / compileSlaSaTestInput).
    pub fn test_codegen(test_filter: Option<String>) -> CompileOptions {
        CompileOptions {
            test_filter,
            prune_for_test_codegen: true,
            load_reachable_imported_bodies_from_registry: true,
            goroutine_coop: false,
            prune_for_entry_function: None,
        }
    }
}

/// Zig `gateDisabled`.
pub fn gate_disabled() -> bool {
    match std::env::var("SLA_NO_FRESHNESS_CACHE") {
        Ok(v) => !v.is_empty() && v != "0" && v != "false",
        Err(_) => false,
    }
}

/// cache_version.zig `mixInto` with this binary's stamp.
fn mix_stamp(hasher: &mut WyhashStream) {
    hasher.update(b"sla-cache-key:");
    hasher.update(&KEY_VERSION.to_le_bytes());
    hasher.update(env!("CARGO_PKG_VERSION").as_bytes());
    hasher.update(b"\x00");
}

/// Zig `optionsDigest`.
fn options_digest(kind: FreshKind, options: &CompileOptions, output_file: Option<&str>) -> u64 {
    let mut hasher = WyhashStream::init(0);
    hasher.update(kind.tag().as_bytes());
    hasher.update(b"\x00");
    match &options.test_filter {
        Some(filter) => {
            hasher.update(b"filter:");
            hasher.update(filter.as_bytes());
        }
        None => hasher.update(b"filter:none"),
    }
    hasher.update(b"\x00");
    hasher.update(&[options.prune_for_test_codegen as u8]);
    hasher.update(&[options.load_reachable_imported_bodies_from_registry as u8]);
    hasher.update(&[options.goroutine_coop as u8]);
    match &options.prune_for_entry_function {
        Some(entry) => {
            hasher.update(b"entry:");
            hasher.update(entry.as_bytes());
        }
        None => hasher.update(b"entry:none"),
    }
    hasher.update(b"\x00");
    if let Some(out) = output_file {
        hasher.update(b"out:");
        hasher.update(out.as_bytes());
    }
    hasher.finish()
}

/// Zig `gateKey`.
fn gate_key(
    file: &str,
    kind: FreshKind,
    options: &CompileOptions,
    output_file: Option<&str>,
) -> u64 {
    let mut hasher = WyhashStream::init(0);
    mix_stamp(&mut hasher);
    hasher.update(file.as_bytes());
    hasher.update(b"\x00");
    hasher.update(&options_digest(kind, options, output_file).to_le_bytes());
    hasher.finish()
}

/// Zig `freshDirPath`.
fn fresh_dir_path(
    file: &str,
    kind: FreshKind,
    options: &CompileOptions,
    output_file: Option<&str>,
    ext: &str,
) -> String {
    let base = file.rsplit('/').next().unwrap_or(file);
    let stem = base.strip_suffix(".sla").unwrap_or(base);
    let key = gate_key(file, kind, options, output_file);
    format!(".sla-cache/fresh/{stem}-{key:x}.{ext}")
}

struct ManifestDep {
    path: String,
    len: u64,
    hash: u64,
}

/// Zig `tryLoadFreshArtifact` (local path). Any doubt → None.
pub fn try_load_fresh_artifact(
    file: &str,
    kind: FreshKind,
    options: &CompileOptions,
    output_file: Option<&str>,
) -> Option<Vec<u8>> {
    if gate_disabled() {
        return None;
    }
    let manifest_path = fresh_dir_path(file, kind, options, output_file, "json");
    let manifest_bytes = read_capped(&manifest_path, MAX_MANIFEST_BYTES)?;
    let (version, m_kind, deps) = parse_manifest(&manifest_bytes)?;
    if version != MANIFEST_VERSION {
        return None;
    }
    if m_kind != kind.tag() {
        return None;
    }
    for dep in &deps {
        let bytes = read_capped(&dep.path, MAX_DEP_BYTES)?;
        if bytes.len() as u64 != dep.len {
            return None;
        }
        if wyhash(0, &bytes) != dep.hash {
            return None;
        }
    }
    let artifact_path = fresh_dir_path(file, kind, options, output_file, "bin");
    read_capped(&artifact_path, MAX_ARTIFACT_BYTES)
}

/// Zig `storeFreshArtifact` (local path). Best effort: failures
/// are silent, exactly like the Zig call sites' `catch {}`.
pub fn store_fresh_artifact(
    file: &str,
    kind: FreshKind,
    options: &CompileOptions,
    output_file: Option<&str>,
    dep_paths: &[String],
    artifact_bytes: &[u8],
) {
    if gate_disabled() {
        return;
    }
    let mut seen: Vec<&str> = Vec::new();
    let mut deps: Vec<ManifestDep> = Vec::new();
    for path in dep_paths {
        if seen.contains(&path.as_str()) {
            continue;
        }
        seen.push(path.as_str());
        let Some(bytes) = read_capped(path, MAX_DEP_BYTES) else {
            continue;
        };
        deps.push(ManifestDep {
            path: path.clone(),
            len: bytes.len() as u64,
            hash: wyhash(0, &bytes),
        });
    }
    if deps.is_empty() {
        return;
    }
    let artifact_path = fresh_dir_path(file, kind, options, output_file, "bin");
    let manifest_path = fresh_dir_path(file, kind, options, output_file, "json");
    if let Some(idx) = artifact_path.rfind('/') {
        let _ = std::fs::create_dir_all(&artifact_path[..idx]);
    }
    if std::fs::write(&artifact_path, artifact_bytes).is_err() {
        return;
    }
    let manifest = manifest_json(kind, &deps);
    let _ = std::fs::write(&manifest_path, manifest);
}

fn read_capped(path: &str, cap: u64) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() > cap {
        return None;
    }
    std::fs::read(path).ok()
}

// -- manifest JSON (std.json Stringify shape for this struct) ---------------

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn manifest_json(kind: FreshKind, deps: &[ManifestDep]) -> String {
    let mut out = String::new();
    out.push_str("{\"version\":");
    out.push_str(&MANIFEST_VERSION.to_string());
    out.push_str(",\"kind\":\"");
    out.push_str(kind.tag());
    out.push_str("\",\"deps\":[");
    for (i, dep) in deps.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"path\":\"");
        out.push_str(&json_escape(&dep.path));
        out.push_str("\",\"len\":");
        out.push_str(&dep.len.to_string());
        out.push_str(",\"hash\":");
        out.push_str(&dep.hash.to_string());
        out.push('}');
    }
    out.push_str("]}");
    out
}

/// Minimal parser for the manifest shape above (the workspace
/// carries no JSON dependency). Returns (version, kind, deps).
fn parse_manifest(bytes: &[u8]) -> Option<(u32, String, Vec<ManifestDep>)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut p = JsonCursor {
        bytes: text.as_bytes(),
        pos: 0,
    };
    p.expect(b'{')?;
    let mut version: Option<u32> = None;
    let mut kind: Option<String> = None;
    let mut deps: Option<Vec<ManifestDep>> = None;
    loop {
        p.skip_ws();
        if p.eat(b'}') {
            break;
        }
        let key = p.string()?;
        p.expect(b':')?;
        match key.as_str() {
            "version" => version = Some(p.number()? as u32),
            "kind" => kind = Some(p.string()?),
            "deps" => {
                deps = Some(p.deps()?);
            }
            _ => {
                p.skip_value()?;
            }
        }
        p.skip_ws();
        if p.eat(b',') {
            continue;
        }
        p.expect(b'}')?;
        break;
    }
    Some((version?, kind?, deps?))
}

struct JsonCursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonCursor<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }
    fn eat(&mut self, b: u8) -> bool {
        self.skip_ws();
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, b: u8) -> Option<()> {
        if self.eat(b) {
            Some(())
        } else {
            None
        }
    }
    fn string(&mut self) -> Option<String> {
        self.skip_ws();
        if self.pos >= self.bytes.len() || self.bytes[self.pos] != b'"' {
            return None;
        }
        self.pos += 1;
        let mut out = String::new();
        loop {
            if self.pos >= self.bytes.len() {
                return None;
            }
            let c = self.bytes[self.pos];
            self.pos += 1;
            match c {
                b'"' => return Some(out),
                b'\\' => {
                    let e = *self.bytes.get(self.pos)?;
                    self.pos += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hex = self.bytes.get(self.pos..self.pos + 4)?;
                            let code =
                                u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                            self.pos += 4;
                        }
                        _ => return None,
                    }
                }
                _ => {
                    // Reassemble UTF-8 by byte (paths here are
                    // valid UTF-8 slices of valid JSON text).
                    let start = self.pos - 1;
                    let len = utf8_len(self.bytes[start]);
                    let end = (start + len).min(self.bytes.len());
                    out.push_str(std::str::from_utf8(&self.bytes[start..end]).ok()?);
                    self.pos = end;
                }
            }
        }
    }
    fn number(&mut self) -> Option<u64> {
        self.skip_ws();
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == start {
            return None;
        }
        std::str::from_utf8(&self.bytes[start..self.pos])
            .ok()?
            .parse()
            .ok()
    }
    fn deps(&mut self) -> Option<Vec<ManifestDep>> {
        let mut out = Vec::new();
        self.expect(b'[')?;
        loop {
            self.skip_ws();
            if self.eat(b']') {
                return Some(out);
            }
            self.expect(b'{')?;
            let (mut path, mut len, mut hash) = (None, None, None);
            loop {
                self.skip_ws();
                if self.eat(b'}') {
                    break;
                }
                let key = self.string()?;
                self.expect(b':')?;
                match key.as_str() {
                    "path" => path = Some(self.string()?),
                    "len" => len = Some(self.number()?),
                    "hash" => hash = Some(self.number()?),
                    _ => self.skip_value()?,
                }
                if self.eat(b',') {
                    continue;
                }
                self.expect(b'}')?;
                break;
            }
            out.push(ManifestDep {
                path: path?,
                len: len?,
                hash: hash?,
            });
            if self.eat(b',') {
                continue;
            }
            self.expect(b']')?;
            return Some(out);
        }
    }
    fn skip_value(&mut self) -> Option<()> {
        self.skip_ws();
        match *self.bytes.get(self.pos)? {
            b'"' => {
                self.string()?;
            }
            b'{' | b'[' => {
                let open = self.bytes[self.pos];
                let close = if open == b'{' { b'}' } else { b']' };
                let mut depth = 0usize;
                loop {
                    let c = *self.bytes.get(self.pos)?;
                    self.pos += 1;
                    if c == b'"' {
                        self.pos -= 1;
                        self.string()?;
                        continue;
                    }
                    if c == open {
                        depth += 1;
                    }
                    if c == close {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
            }
            _ => {
                while self.pos < self.bytes.len()
                    && !matches!(self.bytes[self.pos], b',' | b'}' | b']')
                {
                    self.pos += 1;
                }
            }
        }
        Some(())
    }
}

fn utf8_len(first: u8) -> usize {
    if first < 0x80 {
        1
    } else if first >> 5 == 0b110 {
        2
    } else if first >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_round_trip() {
        let deps = vec![
            ManifestDep {
                path: "src/main.sla".to_string(),
                len: 42,
                hash: 123456789,
            },
            ManifestDep {
                path: "sa_std/fmt.sai".to_string(),
                len: 7,
                hash: 0,
            },
        ];
        let text = manifest_json(FreshKind::Sab, &deps);
        assert_eq!(
            text,
            "{\"version\":1,\"kind\":\"sab\",\"deps\":[{\"path\":\"src/main.sla\",\"len\":42,\"hash\":123456789},{\"path\":\"sa_std/fmt.sai\",\"len\":7,\"hash\":0}]}"
        );
        let (version, kind, parsed) = parse_manifest(text.as_bytes()).unwrap();
        assert_eq!(version, 1);
        assert_eq!(kind, "sab");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].hash, 123456789);
        assert_eq!(parsed[1].path, "sa_std/fmt.sai");
    }

    #[test]
    fn digest_and_key_are_option_sensitive() {
        let a = CompileOptions::defaults();
        let mut b = CompileOptions::defaults();
        b.goroutine_coop = true;
        assert_ne!(
            options_digest(FreshKind::Sab, &a, None),
            options_digest(FreshKind::Sab, &b, None)
        );
        assert_ne!(
            options_digest(FreshKind::Sab, &a, None),
            options_digest(FreshKind::SaText, &a, None)
        );
        assert_ne!(
            options_digest(FreshKind::SaText, &a, Some("x.sa")),
            options_digest(FreshKind::SaText, &a, Some("y.sa"))
        );
        assert_ne!(
            gate_key("a.sla", FreshKind::Sab, &a, None),
            gate_key("b.sla", FreshKind::Sab, &a, None)
        );
        assert!(fresh_dir_path("src/main.sla", FreshKind::Sab, &a, None, "bin")
            .starts_with(".sla-cache/fresh/main-"));
    }

    #[test]
    fn store_then_load_round_trip_and_invalidation() {
        let dir = std::env::temp_dir().join(format!(
            "sla_cli_fresh_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        std::fs::write("main.sla", "fn main() -> i32 {\n    return 0;\n};\n").unwrap();
        let opts = CompileOptions::defaults();
        let deps = vec!["main.sla".to_string()];
        store_fresh_artifact("main.sla", FreshKind::Sab, &opts, None, &deps, b"SABBYTES");
        let hit = try_load_fresh_artifact("main.sla", FreshKind::Sab, &opts, None);
        assert_eq!(hit.as_deref(), Some(b"SABBYTES".as_slice()));
        // Changing the dep invalidates; a different kind misses.
        std::fs::write("main.sla", "fn main() -> i32 {\n    return 1;\n};\n").unwrap();
        assert!(try_load_fresh_artifact("main.sla", FreshKind::Sab, &opts, None).is_none());
        std::env::set_current_dir(prev).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
