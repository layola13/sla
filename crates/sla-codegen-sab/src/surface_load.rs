//! D1 — std surface file location and rule parsing, mirroring
//! `sab_codegen.zig` lines 1273–1589: `readStdSurfaceFile`,
//! `saStdRootLooksValid`, `dupeIfValidSaStdRoot`,
//! `resolveSaStdRoot`, `cachedSaStdRoot`, and the
//! `loadStdSurfaceRules` parser family
//! (`parseStdSurfaceArg`/`parseStdSurfaceArgs`/`parseStdSurfaceDeps`/
//! `parseStdSurfaceOption`/`appendStdSurfaceRule`).
//!
//! The fallback chains are reproduced in Zig's exact order. Two
//! registered behaviours ride along, unchanged in effect:
//!
//! - SAB-D02 (the silent chain): in Zig every failed candidate is
//!   swallowed (`else |_| {}`) and a missing surface file makes
//!   `loadStdSurfaceRules` return with **zero rules** and no trace.
//!   This port keeps the same control flow and the same zero-rule
//!   outcome, but every candidate attempt and the final landing
//!   point are recorded in a [`SurfaceLoadLog`] the caller can
//!   inspect — the D02 porting rule ("record loudly, do not
//!   reproduce the silence"). The log is diagnostic only; no
//!   emitted byte depends on it (SAB-D15 kin).
//! - `resolveSaStdRoot` is the loud counterpart: when no candidate
//!   root validates, Zig returns `error.FileNotFound` and SAB
//!   generation aborts. Mirrored here.
//!
//! Filesystem access goes through [`SurfaceRoots`], an injectable
//! snapshot of every root the Zig chains consult (environment
//! variables, home directory, self-exe directory, cwd, and the
//! build-time baked `sa_std_source_dir`). The baked directory has
//! no Rust-side counterpart yet — it arrives through the
//! `SLA_SAB_BAKED_SA_STD_DIR` environment slot until the
//! coordinator wires a build-time injection (SAB-C §7 note ①);
//! an unset slot behaves exactly like an invalid baked root in
//! Zig: the candidate is tried, rejected, and the chain moves on.

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::types::{StdSurfaceArgKind, StdSurfaceRule, StdSurfaceRuleKind, StdSurfaceRuleOptions};

/// Zig `readFileAlloc` cap in `readStdSurfaceFile` (1 MiB).
const MAX_SURFACE_FILE_BYTES: usize = 1024 * 1024;

/// Every root the D1 fallback chains consult, snapshotted so tests
/// can point the chains at fixture directories.
#[derive(Debug, Clone, Default)]
pub struct SurfaceRoots {
    /// `SLA_STD_DIR` (surface-file chain, first candidate).
    pub sla_std_dir: Option<String>,
    /// `SA_STD_DIR` (sa_std root chain).
    pub sa_std_dir: Option<String>,
    /// `SCI_ROOT` (sa_std root chain, `<root>/sa_std`).
    pub sci_root: Option<String>,
    /// The user's home directory (`host_paths.homeDirectory`).
    pub home: Option<String>,
    /// Directory of the running executable (`selfExePathAlloc`'s
    /// dirname); surface candidates are relative to it.
    pub self_exe_dir: Option<PathBuf>,
    /// Working directory for the relative candidates.
    pub cwd: PathBuf,
    /// Build-time baked `sla_build_options.sa_std_source_dir`
    /// (sa_std root chain, first candidate). See the module docs
    /// for the injection slot.
    pub baked_sa_std_dir: Option<String>,
}

impl SurfaceRoots {
    /// Snapshot the process environment, exactly the inputs the
    /// Zig chains read at call time.
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.is_empty());
        SurfaceRoots {
            sla_std_dir: var("SLA_STD_DIR"),
            sa_std_dir: var("SA_STD_DIR"),
            sci_root: var("SCI_ROOT"),
            home: var("HOME"),
            self_exe_dir: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf())),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            baked_sa_std_dir: var("SLA_SAB_BAKED_SA_STD_DIR"),
        }
    }
}

/// The loud record of a D1 chain walk (SAB-D02 porting rule): one
/// entry per candidate tried, in order, plus where the chain
/// landed. Entries are human-readable diagnostics, never emission
/// inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SurfaceLoadLog {
    pub attempts: Vec<String>,
    /// The path that produced the surface source, when one did.
    pub landed_path: Option<String>,
    /// The sa_std root that validated, when resolution ran.
    pub landed_root: Option<String>,
}

impl SurfaceLoadLog {
    fn tried(&mut self, path: &std::path::Path, outcome: &str) {
        self.attempts.push(format!("{}: {outcome}", path.display()));
    }
}

/// Zig `std.fs.path.join` over `/`-separated components: later
/// components (including `..` segments) are pushed verbatim.
fn join(root: &str, rel: &str) -> PathBuf {
    let mut out = PathBuf::from(root);
    for part in rel.split('/') {
        out.push(part);
    }
    out
}

fn join_path(root: &std::path::Path, rel: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    for part in rel.split('/') {
        out.push(part);
    }
    out
}

/// One surface-file read attempt, Zig semantics: `FileNotFound` /
/// `NotDir` mean "keep walking"; any other error (including the
/// 1 MiB cap) aborts the chain.
fn try_read_surface_file(path: &std::path::Path, log: &mut SurfaceLoadLog) -> Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => {
            if bytes.len() > MAX_SURFACE_FILE_BYTES {
                log.tried(path, "error StreamTooLong (over 1 MiB cap) — chain aborts");
                return Err(Error::InvalidModule(format!(
                    "std surface file {} exceeds the 1 MiB read cap [group D1 surface_load; Zig sab_codegen.zig:1273-1352]",
                    path.display()
                )));
            }
            log.tried(path, "hit");
            log.landed_path = Some(path.display().to_string());
            Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
        }
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            log.tried(path, "miss (FileNotFound/NotDir) — chain continues");
            Ok(None)
        }
        Err(e) => {
            log.tried(path, &format!("error {e} — chain aborts"));
            Err(Error::InvalidModule(format!(
                "reading std surface file {}: {e} [group D1 surface_load; Zig sab_codegen.zig:1273-1352]",
                path.display()
            )))
        }
    }
}

/// Zig `readStdSurfaceFile` (lines 1273–1352): the four-stage
/// fallback chain for `std_surface.sla_meta`. Returns the source
/// and the path it came from; `None` source means every candidate
/// missed — Zig's silent zero-rule precondition, recorded in the
/// log (SAB-D02).
pub fn read_std_surface_file(
    roots: &SurfaceRoots,
) -> Result<(Option<String>, SurfaceLoadLog)> {
    let mut log = SurfaceLoadLog::default();

    // Stage 1: $SLA_STD_DIR/std_surface.sla_meta.
    if let Some(root) = &roots.sla_std_dir {
        let path = join(root, "std_surface.sla_meta");
        if let Some(source) = try_read_surface_file(&path, &mut log)? {
            return Ok((Some(source), log));
        }
    } else {
        log.attempts.push("SLA_STD_DIR: unset — stage skipped".to_string());
    }

    // Stage 2: ~/projects/sa_plugins/sa_plugin_sla/sla_std/…
    if let Some(home) = &roots.home {
        let path = join(
            home,
            "projects/sa_plugins/sa_plugin_sla/sla_std/std_surface.sla_meta",
        );
        if let Some(source) = try_read_surface_file(&path, &mut log)? {
            return Ok((Some(source), log));
        }
    } else {
        log.attempts.push("home directory: unavailable — stage skipped".to_string());
    }

    // Stage 3: self-exe-relative candidates.
    if let Some(exe_dir) = &roots.self_exe_dir {
        for rel in [
            "../../sla_std/std_surface.sla_meta",
            "../sla_std/std_surface.sla_meta",
            "../../sa_plugin_sla/sla_std/std_surface.sla_meta",
        ] {
            let path = join_path(exe_dir, rel);
            if let Some(source) = try_read_surface_file(&path, &mut log)? {
                return Ok((Some(source), log));
            }
        }
    } else {
        log.attempts.push("self-exe path: unavailable — stage skipped".to_string());
    }

    // Stage 4: cwd-relative candidates (+ one hardcoded vscode path).
    for rel in [
        "sla_std/std_surface.sla_meta",
        "sa_plugins/sa_plugin_sla/sla_std/std_surface.sla_meta",
        "../sa_plugins/sa_plugin_sla/sla_std/std_surface.sla_meta",
        "/home/vscode/projects/sa_plugins/sa_plugin_sla/sla_std/std_surface.sla_meta",
    ] {
        let path = if rel.starts_with('/') {
            PathBuf::from(rel)
        } else {
            join_path(&roots.cwd, rel)
        };
        if let Some(source) = try_read_surface_file(&path, &mut log)? {
            return Ok((Some(source), log));
        }
    }

    log.attempts
        .push("std_surface.sla_meta: no candidate hit — zero rules (SAB-D02)".to_string());
    Ok((None, log))
}

/// The four files a candidate sa_std root must contain
/// (Zig `saStdRootLooksValid`, lines 1354–1372).
pub const SA_STD_REQUIRED_FILES: [&str; 4] = [
    "core/sa_core.sa",
    "core/option.sa",
    "core/result.sa",
    "io/print.sai",
];

/// Zig `saStdRootLooksValid`: every required file openable.
/// `FileNotFound`/`NotDir` on any one invalidates the root; other
/// errors abort (Zig `else => return err`).
pub fn sa_std_root_looks_valid(root: &str, log: &mut SurfaceLoadLog) -> Result<bool> {
    for rel in SA_STD_REQUIRED_FILES {
        let path = join(root, rel);
        match std::fs::File::open(&path) {
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                log.tried(&PathBuf::from(root), &format!("invalid (missing {rel})"));
                return Ok(false);
            }
            Err(e) => {
                return Err(Error::InvalidModule(format!(
                    "probing sa_std root {root} at {rel}: {e} [group D1 surface_load; Zig sab_codegen.zig:1354-1372]"
                )))
            }
        }
    }
    log.tried(&PathBuf::from(root), "valid sa_std root");
    Ok(true)
}

/// Zig `dupeIfValidSaStdRoot`.
fn dupe_if_valid(root: &str, log: &mut SurfaceLoadLog) -> Result<Option<String>> {
    if sa_std_root_looks_valid(root, log)? {
        Ok(Some(root.to_string()))
    } else {
        Ok(None)
    }
}

/// Zig `resolveSaStdRoot` (lines 1374–1428): baked dir, then
/// `SA_STD_DIR`, then `SCI_ROOT/sa_std`, then the two home roots,
/// then the cwd candidate list. Total failure is Zig's
/// `error.FileNotFound` — loud, unlike the surface-file chain.
pub fn resolve_sa_std_root(roots: &SurfaceRoots) -> Result<(String, SurfaceLoadLog)> {
    let mut log = SurfaceLoadLog::default();

    if let Some(baked) = &roots.baked_sa_std_dir {
        if let Some(root) = dupe_if_valid(baked, &mut log)? {
            log.landed_root = Some(root.clone());
            return Ok((root, log));
        }
    } else {
        log.attempts
            .push("baked sa_std_source_dir: no Rust-side injection yet — candidate skipped".to_string());
    }
    if let Some(env_root) = &roots.sa_std_dir {
        if let Some(root) = dupe_if_valid(env_root, &mut log)? {
            log.landed_root = Some(root.clone());
            return Ok((root, log));
        }
    }
    if let Some(sci_root) = &roots.sci_root {
        let candidate = join(sci_root, "sa_std").display().to_string();
        if let Some(root) = dupe_if_valid(&candidate, &mut log)? {
            log.landed_root = Some(root.clone());
            return Ok((root, log));
        }
    }
    if let Some(home) = &roots.home {
        for rel in ["projects/sci/sa_std", ".sa/std"] {
            let candidate = join(home, rel).display().to_string();
            if let Some(root) = dupe_if_valid(&candidate, &mut log)? {
                log.landed_root = Some(root.clone());
                return Ok((root, log));
            }
        }
    }
    for rel in [
        "sa_std",
        "sci/sa_std",
        "../sci/sa_std",
        "../../sci/sa_std",
        "/home/vscode/projects/sci/sa_std",
        "/home/vscode/.sa/std",
    ] {
        let candidate = if rel.starts_with('/') {
            rel.to_string()
        } else {
            join_path(&roots.cwd, rel).display().to_string()
        };
        if let Some(root) = dupe_if_valid(&candidate, &mut log)? {
            log.landed_root = Some(root.clone());
            return Ok((root, log));
        }
    }
    Err(Error::InvalidModule(
        "FileNotFound: no candidate sa_std root validated (four required files) [group D1 surface_load; Zig sab_codegen.zig:1374-1428]".to_string(),
    ))
}

// -- rule parsing (Zig 1440–1589) -----------------------------------

fn unsupported_parse(what: &str) -> Error {
    Error::UnsupportedSabDirectFeature(format!(
        "{what} [group D1 surface_load; Zig sab_codegen.zig:1440-1589]"
    ))
}

/// Zig `parseStdSurfaceArg` (one comma item).
pub fn parse_std_surface_arg(text: &str) -> Result<StdSurfaceArgKind> {
    Ok(match text {
        "out" => StdSurfaceArgKind::Out,
        "ok" => StdSurfaceArgKind::Ok,
        "receiver" => StdSurfaceArgKind::Receiver,
        "value" => StdSurfaceArgKind::Value,
        "index" => StdSurfaceArgKind::Index,
        "elem_size" => StdSurfaceArgKind::ElemSize,
        "elem_ty" => StdSurfaceArgKind::ElemTy,
        _ => return Err(unsupported_parse(&format!("unknown std surface arg `{text}`"))),
    })
}

/// Zig `parseStdSurfaceArgs`: comma-split, trimmed, empties skipped.
pub fn parse_std_surface_args(text: &str) -> Result<Vec<StdSurfaceArgKind>> {
    let mut args = Vec::new();
    for raw in text.split(',') {
        let item = raw.trim_matches(|c| matches!(c, ' ' | '\t' | '\r'));
        if item.is_empty() {
            continue;
        }
        args.push(parse_std_surface_arg(item)?);
    }
    Ok(args)
}

/// Zig `parseStdSurfaceDeps`: `deps=a,b,c` payload, or empty for
/// an absent option. A present-but-misspelled option is Zig's
/// `UnsupportedSabDirectFeature`.
pub fn parse_std_surface_deps(text: Option<&str>) -> Result<Vec<String>> {
    let raw = match text {
        None => return Ok(Vec::new()),
        Some(raw) => raw,
    };
    let payload = raw
        .strip_prefix("deps=")
        .ok_or_else(|| unsupported_parse(&format!("std surface option `{raw}` is not deps=")))?;
    let mut deps = Vec::new();
    for raw_item in payload.split(',') {
        let item = raw_item.trim_matches(|c| matches!(c, ' ' | '\t' | '\r'));
        if item.is_empty() {
            continue;
        }
        deps.push(item.to_string());
    }
    Ok(deps)
}

/// Zig `parseStdSurfaceOption`: `deps=` / `panic=` / `consume_value`.
pub fn parse_std_surface_option(
    text: &str,
    options: &mut StdSurfaceRuleOptions,
) -> Result<()> {
    if text.starts_with("deps=") {
        options.deps = parse_std_surface_deps(Some(text))?;
        return Ok(());
    }
    if let Some(payload) = text.strip_prefix("panic=") {
        options.panic_code = Some(payload.parse::<i64>().map_err(|_| {
            unsupported_parse(&format!("std surface panic= value `{payload}` is not an i64"))
        })?);
        return Ok(());
    }
    if text == "consume_value" {
        options.consume_value = true;
        return Ok(());
    }
    Err(unsupported_parse(&format!("unknown std surface option `{text}`")))
}

/// Zig `appendStdSurfaceRule` + `loadStdSurfaceRules` line loop:
/// parse a whole `std_surface.sla_meta` source into rules, in file
/// order. Blank lines and `#` comments are skipped; every other
/// line must match one of the eight kind shapes exactly, including
/// the swapped type/member order of `function`/`constructor` lines
/// (member first) versus `associated`/`method` lines (type first).
pub fn parse_std_surface_rules(source: &str) -> Result<Vec<StdSurfaceRule>> {
    let mut rules = Vec::new();
    for raw_line in source.split('\n') {
        let line = raw_line.trim_matches(|c| matches!(c, ' ' | '\t' | '\r'));
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split(|c| matches!(c, ' ' | '\t')).filter(|t| !t.is_empty());
        let raw_kind = match parts.next() {
            Some(kind) => kind,
            None => continue,
        };
        fn next<'a>(parts: &mut dyn Iterator<Item = &'a str>) -> Result<&'a str> {
            parts
                .next()
                .ok_or_else(|| unsupported_parse("truncated std surface rule line"))
        }
        let (kind, type_name, member_name): (StdSurfaceRuleKind, &str, Option<&str>) =
            match raw_kind {
                "associated" | "method" | "fallible_method" => {
                    let ty = next(&mut parts)?;
                    let member = next(&mut parts)?;
                    let kind = match raw_kind {
                        "associated" => StdSurfaceRuleKind::Associated,
                        "method" => StdSurfaceRuleKind::Method,
                        _ => StdSurfaceRuleKind::FallibleMethod,
                    };
                    (kind, ty, Some(member))
                }
                "function" | "constructor" => {
                    // Zig reads member first, then type, for these
                    // two kinds — the swap is in the original.
                    let member = next(&mut parts)?;
                    let ty = next(&mut parts)?;
                    let kind = if raw_kind == "function" {
                        StdSurfaceRuleKind::Function
                    } else {
                        StdSurfaceRuleKind::Constructor
                    };
                    (kind, ty, Some(member))
                }
                "index" | "index_address" | "index_assign" => {
                    let ty = next(&mut parts)?;
                    let kind = match raw_kind {
                        "index" => StdSurfaceRuleKind::Index,
                        "index_address" => StdSurfaceRuleKind::IndexAddress,
                        _ => StdSurfaceRuleKind::IndexAssign,
                    };
                    (kind, ty, None)
                }
                _ => {
                    return Err(unsupported_parse(&format!(
                        "unknown std surface rule kind `{raw_kind}`"
                    )))
                }
            };
        let import_path = next(&mut parts)?;
        let macro_name = next(&mut parts)?;
        let arg_text = parts.next().unwrap_or("");
        let mut options = StdSurfaceRuleOptions::default();
        for option in parts {
            parse_std_surface_option(option, &mut options)?;
        }
        if kind == StdSurfaceRuleKind::FallibleMethod && options.panic_code.is_none() {
            return Err(unsupported_parse(
                "fallible_method std surface rule without panic=",
            ));
        }
        rules.push(StdSurfaceRule {
            kind,
            type_name: type_name.to_string(),
            member_name: member_name.map(|m| m.to_string()),
            import_path: import_path.to_string(),
            macro_name: macro_name.to_string(),
            args: parse_std_surface_args(arg_text)?,
            deps: options.deps,
            panic_code: options.panic_code,
            consume_value: options.consume_value,
        });
    }
    Ok(rules)
}

/// Zig `loadStdSurfaceRules` (lines 1551–1589) as a state
/// transition over the rule table: a non-empty table is left
/// alone; a missing file leaves the table empty (the SAB-D02
/// zero-rule outcome, loudly logged); a found file is parsed in
/// full. Returns the walk log either way.
pub fn load_std_surface_rules(
    rules: &mut Vec<StdSurfaceRule>,
    roots: &SurfaceRoots,
) -> Result<SurfaceLoadLog> {
    if !rules.is_empty() {
        let mut log = SurfaceLoadLog::default();
        log.attempts
            .push("std surface rules already loaded — load is a no-op (Zig guard)".to_string());
        return Ok(log);
    }
    let (source, mut log) = read_std_surface_file(roots)?;
    let source = match source {
        Some(source) => source,
        None => return Ok(log), // zero rules, as in Zig; log says so.
    };
    let parsed = parse_std_surface_rules(&source)?;
    log.attempts
        .push(format!("parsed {} std surface rules", parsed.len()));
    rules.extend(parsed);
    Ok(log)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::StdSurfaceArgKind as Arg;

    /// Excerpted verbatim from the venue's current-head
    /// `sla_std/std_surface.sla_meta` (one line per kind family).
    const META_EXCERPT: &str = "\
# Sla std surface lowering metadata.
associated Vec new sa_std/vec.sa VEC_NEW out deps=sa_vec_new
associated Box new sa_std/core/box.sa BOX_NEW out,value consume_value
method Vec push sa_std/vec.sa VEC_PUSH receiver,value,elem_size deps=sa_vec_push,sa_mem_copy
function len Vec sa_std/vec.sa VEC_LEN out,receiver deps=sa_vec_len
fallible_method Vec remove sa_std/vec.sa VEC_REMOVE ok,out,receiver,index,elem_size deps=sa_vec_try_remove panic=86
index Vec sa_std/vec.sa VEC_GET_TYPED_{elem_ty} out,receiver,index,elem_size
index_address Vec sa_std/vec.sa VEC_GET_MUT_PTR_U64 out,receiver,index
index_assign Vec sa_std/vec.sa VEC_SET_TYPED receiver,index,value,elem_size,elem_ty
constructor Some Option sa_std/core/option.sa OPTION_NEW_SOME out,value
constructor None Option sa_std/core/option.sa OPTION_NEW_NONE out
";

    #[test]
    fn parse_meta_excerpt_all_kinds() {
        let rules = parse_std_surface_rules(META_EXCERPT).expect("excerpt parses");
        assert_eq!(rules.len(), 10);
        let new = &rules[0];
        assert_eq!(new.kind, StdSurfaceRuleKind::Associated);
        assert_eq!(new.type_name, "Vec");
        assert_eq!(new.member_name.as_deref(), Some("new"));
        assert_eq!(new.deps, vec!["sa_vec_new".to_string()]);
        let boxed = &rules[1];
        assert!(boxed.consume_value);
        assert_eq!(boxed.args, vec![Arg::Out, Arg::Value]);
        // function lines swap: member `len`, type `Vec`.
        let len = &rules[3];
        assert_eq!(len.kind, StdSurfaceRuleKind::Function);
        assert_eq!(len.type_name, "Vec");
        assert_eq!(len.member_name.as_deref(), Some("len"));
        let remove = &rules[4];
        assert_eq!(remove.kind, StdSurfaceRuleKind::FallibleMethod);
        assert_eq!(remove.panic_code, Some(86));
        let index = &rules[5];
        assert_eq!(index.kind, StdSurfaceRuleKind::Index);
        assert_eq!(index.member_name, None);
        assert_eq!(index.macro_name, "VEC_GET_TYPED_{elem_ty}");
        let none = &rules[9];
        assert_eq!(none.kind, StdSurfaceRuleKind::Constructor);
        assert_eq!(none.type_name, "Option");
        assert_eq!(none.member_name.as_deref(), Some("None"));
        assert_eq!(none.args, vec![Arg::Out]);
    }

    #[test]
    fn parse_rejects_unknown_kind_arg_and_option() {
        assert!(parse_std_surface_rules("bogus Vec new sa_std/vec.sa VEC_NEW out\n").is_err());
        assert!(parse_std_surface_rules("method Vec push sa_std/vec.sa VEC_PUSH bogus\n").is_err());
        assert!(parse_std_surface_rules(
            "method Vec push sa_std/vec.sa VEC_PUSH receiver wat=1\n"
        )
        .is_err());
        // fallible_method without panic= is Zig's hard error.
        assert!(parse_std_surface_rules(
            "fallible_method Vec remove sa_std/vec.sa VEC_REMOVE ok,out\n"
        )
        .is_err());
    }

    fn roots_at(cwd: &std::path::Path) -> SurfaceRoots {
        SurfaceRoots {
            cwd: cwd.to_path_buf(),
            ..Default::default()
        }
    }

    #[test]
    fn fallback_chain_lands_on_cwd_candidate_and_logs() {
        let dir = std::env::temp_dir().join(format!("sab-d1-{}", std::process::id()));
        let sla_std = dir.join("sla_std");
        std::fs::create_dir_all(&sla_std).unwrap();
        std::fs::write(sla_std.join("std_surface.sla_meta"), META_EXCERPT).unwrap();
        let (source, log) = read_std_surface_file(&roots_at(&dir)).expect("chain runs");
        assert!(source.is_some());
        assert!(log.landed_path.as_deref().unwrap().contains("sla_std"));
        assert!(log.attempts.len() >= 4, "every stage recorded: {:?}", log.attempts);
        // Zero-rule outcome: an empty tree misses everywhere.
        let empty = std::env::temp_dir().join(format!("sab-d1-empty-{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        let (source, log) = read_std_surface_file(&roots_at(&empty)).expect("chain runs");
        assert!(source.is_none());
        assert!(log.attempts.last().unwrap().contains("zero rules"));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::remove_dir_all(&empty).ok();
    }

    #[test]
    fn sla_std_dir_stage_wins_over_cwd() {
        let base = std::env::temp_dir().join(format!("sab-d1-win-{}", std::process::id()));
        let env_root = base.join("envroot");
        let cwd_root = base.join("cwdroot").join("sla_std");
        std::fs::create_dir_all(&env_root).unwrap();
        std::fs::create_dir_all(&cwd_root).unwrap();
        std::fs::write(env_root.join("std_surface.sla_meta"), "method A b p M out\n").unwrap();
        std::fs::write(cwd_root.join("std_surface.sla_meta"), "method C d p M out\n").unwrap();
        let mut roots = roots_at(&base.join("cwdroot"));
        roots.sla_std_dir = Some(env_root.display().to_string());
        let (source, log) = read_std_surface_file(&roots).expect("chain runs");
        assert!(source.unwrap().contains("method A b"));
        assert_eq!(log.attempts.len(), 1, "first stage hit stops the chain");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn sa_std_root_validation_and_resolution() {
        let base = std::env::temp_dir().join(format!("sab-d1-root-{}", std::process::id()));
        let root = base.join("sa_std");
        for rel in SA_STD_REQUIRED_FILES {
            let path = join(&root.display().to_string(), rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "").unwrap();
        }
        let mut roots = roots_at(&base);
        // cwd candidate `sa_std` under the fixture cwd validates.
        let (resolved, log) = resolve_sa_std_root(&roots).expect("root resolves");
        assert!(resolved.ends_with("sa_std"));
        assert_eq!(log.landed_root.as_deref(), Some(resolved.as_str()));
        // Removing one required file invalidates every candidate.
        std::fs::remove_file(join(&root.display().to_string(), "io/print.sai")).unwrap();
        roots.baked_sa_std_dir = None;
        assert!(resolve_sa_std_root(&roots).is_err());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn load_is_noop_when_rules_present() {
        let mut rules = parse_std_surface_rules(META_EXCERPT).unwrap();
        let before = rules.len();
        let dir = std::env::temp_dir();
        let log = load_std_surface_rules(&mut rules, &roots_at(&dir)).expect("load runs");
        assert_eq!(rules.len(), before);
        assert!(log.attempts[0].contains("already loaded"));
    }
}
