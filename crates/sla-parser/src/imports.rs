//! Import path resolution, mirrored from
//! `sa_plugin_sla/src/plugin_imports.zig` (446 lines).
//!
//! Placement note (coordination ruling D-A2 follow-up): the
//! resolution machinery is consumed by BOTH the sla-plugin wiring
//! layer and sla-oracle, and sla-parser is the only crate both
//! already depend on — so it lives here, in a module segregated from
//! the pure parser core (this module does filesystem IO; the parser
//! proper still does none, cf. D-P11). The orchestration that uses
//! it is `contract_load`; the checker-facing registration wrapper is
//! `sla_plugin::contracts`.
//!
//! Deliberate omissions, following the standing D-C3 registration:
//! the Zig fallback chains end in a home-directory guess
//! (`~/projects/sci/sa_std`, `~/projects/sa_plugins/...`) and
//! hard-coded personal machine paths (`/home/vscode/projects/...`).
//! Those are NOT ported. What is ported, in Zig's order: ancestor
//! roots, the `SA_STD_DIR` / `SLA_STD_DIR` / `SCI_ROOT` env roots,
//! the plugin-home resolution, and plain relative resolution.
//! Workspace-package imports (`workspace.zig`) are not ported: such
//! paths fall through to relative resolution and fail loudly with
//! FileNotFound, exactly as a Zig workspace miss does (see
//! docs/divergences/contracts.md INT2-5).

use std::path::{Path, PathBuf};

/// Zig `plugin_imports.max_import_bytes`.
pub const MAX_IMPORT_BYTES: u64 = 16 * 1024 * 1024;

/// Zig `ResolvedImport`: the canonical on-disk `path`, the
/// `output_path` the import is reported under (the import string as
/// written for std/plugin imports), and the file `source`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    pub path: String,
    pub output_path: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    FileNotFound(String),
    Io { path: String, message: String },
    TooLarge(String),
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::FileNotFound(p) => write!(f, "FileNotFound: {p}"),
            ImportError::Io { path, message } => write!(f, "IO error at {path}: {message}"),
            ImportError::TooLarge(p) => write!(f, "import exceeds max bytes: {p}"),
        }
    }
}

impl std::error::Error for ImportError {}

fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

pub fn real_path(path: &Path) -> String {
    match std::fs::canonicalize(path) {
        Ok(p) => normalize_slashes(&p.to_string_lossy()),
        Err(_) => normalize_slashes(&path.to_string_lossy()),
    }
}

fn env_value(name: &str) -> Option<String> {
    std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
}

/// Zig `host_paths.homeDirectory`: USERPROFILE, then HOME.
fn home_directory() -> Option<String> {
    for name in ["USERPROFILE", "HOME"] {
        if let Some(v) = env_value(name) {
            if !v.is_empty() {
                return Some(v);
            }
        }
    }
    None
}

pub fn is_sa_std_import(path: &str) -> bool {
    path == "sa_std" || path.starts_with("sa_std/")
}

pub fn is_sla_std_import(path: &str) -> bool {
    path == "sla_std" || path.starts_with("sla_std/")
}

pub fn is_plugin_import(path: &str) -> bool {
    path.starts_with("plugin/")
}

pub fn is_glob_import_path(path: &str) -> bool {
    path.contains('*')
}

/// Zig `globNameMatches`: zero or one `*`; two or more never match.
pub fn glob_name_matches(pattern: &str, name: &str) -> bool {
    let Some(star) = pattern.find('*') else {
        return pattern == name;
    };
    if pattern[star + 1..].contains('*') {
        return false;
    }
    let prefix = &pattern[..star];
    let suffix = &pattern[star + 1..];
    name.len() >= prefix.len() + suffix.len()
        && name.starts_with(prefix)
        && name.ends_with(suffix)
}

/// Zig `readImportFileIfExistsWithOutputPath`: missing / not-a-file
/// paths yield `None`; other IO failures are errors.
pub fn read_import_file_if_exists(
    path: &Path,
    output_path: &str,
) -> Result<Option<ResolvedImport>, ImportError> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(ImportError::Io {
                path: path.to_string_lossy().into_owned(),
                message: e.to_string(),
            })
        }
    };
    if !meta.is_file() {
        return Ok(None);
    }
    if meta.len() > MAX_IMPORT_BYTES {
        return Err(ImportError::TooLarge(
            path.to_string_lossy().into_owned(),
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| ImportError::Io {
        path: path.to_string_lossy().into_owned(),
        message: e.to_string(),
    })?;
    Ok(Some(ResolvedImport {
        path: real_path(path),
        output_path: output_path.to_string(),
        source: String::from_utf8_lossy(&bytes).into_owned(),
    }))
}

/// Zig `readImportFromRoot`.
fn read_import_from_root(
    root: &Path,
    rel_path: &str,
    output_path: &str,
) -> Result<Option<ResolvedImport>, ImportError> {
    if rel_path.is_empty() {
        return Ok(None);
    }
    read_import_file_if_exists(&root.join(rel_path), output_path)
}

/// Zig `readImportFromAncestorRoots`: try `current/<root>/<rel>` for
/// every ancestor directory of `base_dir`, starting at its realpath.
fn read_import_from_ancestor_roots(
    base_dir: &Path,
    rel_path: &str,
    output_path: &str,
    roots: &[&str],
) -> Result<Option<ResolvedImport>, ImportError> {
    let mut current = PathBuf::from(real_path(base_dir));
    loop {
        for root in roots {
            let root_path = current.join(root);
            if let Some(resolved) = read_import_from_root(&root_path, rel_path, output_path)? {
                return Ok(Some(resolved));
            }
        }
        let Some(parent) = current.parent().map(Path::to_path_buf) else {
            break;
        };
        if parent == current {
            break;
        }
        current = parent;
    }
    Ok(None)
}

/// Zig `resolveSaStdImportFromBase` (minus the D-C3-omitted tail).
fn resolve_sa_std_import_from_base(
    base_dir: &Path,
    import_path: &str,
) -> Result<Option<ResolvedImport>, ImportError> {
    if !is_sa_std_import(import_path) || import_path == "sa_std" {
        return Ok(None);
    }
    let rel_path = &import_path["sa_std/".len()..];
    if let Some(r) = read_import_from_ancestor_roots(
        base_dir,
        rel_path,
        import_path,
        &["sa_std", "sci/sa_std"],
    )? {
        return Ok(Some(r));
    }
    if let Some(env_root) = env_value("SA_STD_DIR") {
        if let Some(r) = read_import_from_root(Path::new(&env_root), rel_path, import_path)? {
            return Ok(Some(r));
        }
    }
    if let Some(sci_root) = env_value("SCI_ROOT") {
        let root = Path::new(&sci_root).join("sa_std");
        if let Some(r) = read_import_from_root(&root, rel_path, import_path)? {
            return Ok(Some(r));
        }
    }
    Ok(None)
}

/// Zig `resolveSlaStdImportFromBase` (minus the D-C3-omitted tail).
fn resolve_sla_std_import_from_base(
    base_dir: &Path,
    import_path: &str,
) -> Result<Option<ResolvedImport>, ImportError> {
    if !is_sla_std_import(import_path) || import_path == "sla_std" {
        return Ok(None);
    }
    let rel_path = &import_path["sla_std/".len()..];
    if let Some(r) = read_import_from_ancestor_roots(
        base_dir,
        rel_path,
        import_path,
        &["sla_std", "sa_plugin_sla/sla_std", "sa_plugins/sa_plugin_sla/sla_std"],
    )? {
        return Ok(Some(r));
    }
    if let Some(env_root) = env_value("SLA_STD_DIR") {
        if let Some(r) = read_import_from_root(Path::new(&env_root), rel_path, import_path)? {
            return Ok(Some(r));
        }
    }
    Ok(None)
}

/// Zig `pluginsHomeDir`: SA_PLUGINS_HOME, else XDG_DATA_HOME, else
/// `~/.local/share` — each suffixed as in the original.
fn plugins_home_dir() -> Option<PathBuf> {
    if let Some(configured) = env_value("SA_PLUGINS_HOME") {
        return Some(PathBuf::from(configured));
    }
    if let Some(xdg) = env_value("XDG_DATA_HOME") {
        return Some(Path::new(&xdg).join("sa_plugins"));
    }
    home_directory().map(|home| Path::new(&home).join(".local").join("share").join("sa_plugins"))
}

/// Zig `resolvePluginImport`: `plugin/<name>/<relpath>` against
/// `<plugins-home>/installed/<name>/current/sa`.
fn resolve_plugin_import(
    import_path: &str,
) -> Result<Option<ResolvedImport>, ImportError> {
    if !is_plugin_import(import_path) {
        return Ok(None);
    }
    let rest = &import_path["plugin/".len()..];
    let Some(slash) = rest.find('/') else {
        return Ok(None);
    };
    if slash == 0 || slash + 1 >= rest.len() {
        return Ok(None);
    }
    let (name, rel_path) = (&rest[..slash], &rest[slash + 1..]);
    let Some(home) = plugins_home_dir() else {
        return Ok(None);
    };
    let root = home.join("installed").join(name).join("current").join("sa");
    read_import_from_root(&root, rel_path, import_path)
}

fn is_explicit_file_import_path(path: &str) -> bool {
    Path::new(path).is_absolute()
        || path.starts_with("./")
        || path.starts_with("../")
        || path == "."
        || path == ".."
}

/// Zig `resolveImportFile`, in the original's order: sla_std,
/// sa_std, plugin, workspace (not ported — falls through), then the
/// base-dir-relative candidate, then the path as written.
pub fn resolve_import_file(
    base_dir: &Path,
    raw_import_path: &str,
) -> Result<ResolvedImport, ImportError> {
    if let Some(r) = resolve_sla_std_import_from_base(base_dir, raw_import_path)? {
        return Ok(r);
    }
    if let Some(r) = resolve_sa_std_import_from_base(base_dir, raw_import_path)? {
        return Ok(r);
    }
    if let Some(r) = resolve_plugin_import(raw_import_path)? {
        return Ok(r);
    }
    let _ = is_explicit_file_import_path(raw_import_path); // workspace gate shares this test in Zig
    let candidate = if Path::new(raw_import_path).is_absolute() {
        PathBuf::from(raw_import_path)
    } else {
        base_dir.join(raw_import_path)
    };
    if let Some(r) = read_import_file_if_exists(&candidate, &candidate.to_string_lossy())? {
        return Ok(r);
    }
    if !Path::new(raw_import_path).is_absolute() {
        if let Some(r) =
            read_import_file_if_exists(Path::new(raw_import_path), raw_import_path)?
        {
            return Ok(r);
        }
    }
    Err(ImportError::FileNotFound(raw_import_path.to_string()))
}

/// Zig `resolveImportFiles` (+ `appendResolvedImportFiles`): glob
/// patterns expand over a sorted directory listing; the result list
/// is sorted by `output_path` (`resolvedImportLessThan`).
pub fn resolve_import_files(
    base_dir: &Path,
    raw_import_path: &str,
    exclude_path: Option<&str>,
) -> Result<Vec<ResolvedImport>, ImportError> {
    let mut out: Vec<ResolvedImport> = Vec::new();
    if !is_glob_import_path(raw_import_path) {
        let resolved = resolve_import_file(base_dir, raw_import_path)?;
        if let Some(exclude) = exclude_path {
            if resolved.path == exclude {
                return Ok(out);
            }
        }
        out.push(resolved);
        return Ok(out);
    }
    if is_sa_std_import(raw_import_path)
        || is_sla_std_import(raw_import_path)
        || is_plugin_import(raw_import_path)
    {
        return Err(ImportError::FileNotFound(raw_import_path.to_string()));
    }
    let pattern_path = if Path::new(raw_import_path).is_absolute() {
        PathBuf::from(raw_import_path)
    } else {
        base_dir.join(raw_import_path)
    };
    let dir_path = pattern_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let pattern_name = pattern_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut matched_paths: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(&dir_path).map_err(|e| ImportError::Io {
        path: dir_path.to_string_lossy().into_owned(),
        message: e.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| ImportError::Io {
            path: dir_path.to_string_lossy().into_owned(),
            message: e.to_string(),
        })?;
        let ft = entry.file_type().map_err(|e| ImportError::Io {
            path: dir_path.to_string_lossy().into_owned(),
            message: e.to_string(),
        })?;
        if !ft.is_file() && !ft.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !glob_name_matches(&pattern_name, &name) {
            continue;
        }
        matched_paths.push(
            dir_path
                .join(&name)
                .to_string_lossy()
                .into_owned(),
        );
    }
    matched_paths.sort();
    let mut matched_count = 0usize;
    for candidate in &matched_paths {
        if let Some(resolved) =
            read_import_file_if_exists(Path::new(candidate), candidate)?
        {
            if let Some(exclude) = exclude_path {
                if resolved.path == exclude {
                    continue;
                }
            }
            out.push(resolved);
            matched_count += 1;
        }
    }
    if matched_count == 0 {
        return Err(ImportError::FileNotFound(raw_import_path.to_string()));
    }
    out.sort_by(|a, b| a.output_path.cmp(&b.output_path));
    Ok(out)
}

/// Zig `importPathFromLine`: the `@import` path of one source line,
/// quoted (with `\` escapes honored while scanning) or bare.
pub fn import_path_from_line(raw_line: &str) -> Option<String> {
    let line = raw_line.trim_matches(|c| c == ' ' || c == '\t' || c == '\r');
    if !line.starts_with("@import") {
        return None;
    }
    let mut rest = line["@import".len()..].trim_matches(|c| c == ' ' || c == '\t');
    if rest.is_empty() {
        return None;
    }
    if !rest.starts_with('"') {
        if let Some(comment_idx) = rest.find("//") {
            rest = &rest[..comment_idx];
        }
        rest = rest.trim_matches(|c| c == ' ' || c == '\t' || c == '\r' || c == ';');
        if rest.is_empty() {
            return None;
        }
        return Some(rest.to_string());
    }
    if rest.len() < 2 {
        return None;
    }
    let inner = &rest[1..];
    let bytes = inner.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        if bytes[idx] == b'\\' {
            idx += 2;
            continue;
        }
        if bytes[idx] == b'"' {
            return Some(inner[..idx].to_string());
        }
        idx += 1;
    }
    None
}

/// Zig `expandedSourceMayContainImports`.
pub fn expanded_source_may_contain_imports(expanded_source: &str) -> bool {
    expanded_source.contains("@import")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_path_from_line_shapes() {
        assert_eq!(
            import_path_from_line("  @import \"sa_std/fmt.sai\""),
            Some("sa_std/fmt.sai".to_string())
        );
        assert_eq!(
            import_path_from_line("@import \"core/slice.sal\""),
            Some("core/slice.sal".to_string())
        );
        assert_eq!(import_path_from_line("fn main() {}"), None);
        assert_eq!(
            import_path_from_line("@import bare/path.sai;"),
            Some("bare/path.sai".to_string())
        );
        assert_eq!(import_path_from_line("@import \"unterminated"), None);
    }

    #[test]
    fn glob_matching_single_star_only() {
        assert!(glob_name_matches("*.sai", "fmt.sai"));
        assert!(!glob_name_matches("*.sai", "fmt.sa"));
        assert!(glob_name_matches("vec*", "vec"));
        assert!(!glob_name_matches("a*b*c", "abc"));
        assert!(glob_name_matches("exact", "exact"));
    }

    #[test]
    fn relative_resolution_and_exclusion() {
        let dir = std::env::temp_dir().join(format!(
            "sla_imports_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("a.sai"), "@extern f() -> void\n").unwrap();
        let resolved = resolve_import_file(&dir, "sub/a.sai").unwrap();
        assert_eq!(resolved.source, "@extern f() -> void\n");
        assert!(resolved.path.ends_with("sub/a.sai"));
        // Excluding the resolved file itself yields an empty list.
        let excluded =
            resolve_import_files(&dir, "sub/a.sai", Some(resolved.path.as_str())).unwrap();
        assert!(excluded.is_empty());
        // Missing files are a loud FileNotFound.
        assert!(matches!(
            resolve_import_file(&dir, "sub/missing.sai"),
            Err(ImportError::FileNotFound(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
