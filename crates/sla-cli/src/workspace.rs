//! Workspace resolution — mirror of `workspace.zig` (venue head
//! 2cc12ea8): `sa.mod` manifest parsing (package / workspace
//! members / default_member), nearest-manifest anchoring, ancestor
//! workspace climb, and member selection, plus the two call-site
//! wrappers used by the pipeline commands
//! (`resolveWorkspaceSourcePath` / `resolveSlaInputFile` in
//! plugin_compile.zig) with their exact stderr lines.
//!
//! Error mapping follows plugin_compile.zig: FileNotFound /
//! UnknownPackage / MissingDefaultMember / InvalidFormat are
//! printed by the wrapper; anything else (DuplicateEntry,
//! InvalidPath, IO) propagates as a hard failure, exactly like the
//! Zig `else => return err` arms.

use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveError {
    FileNotFound,
    InvalidFormat,
    InvalidPath,
    DuplicateEntry,
    UnknownPackage,
    MissingDefaultMember,
}

#[derive(Debug, Default)]
struct WorkspaceDecl {
    members: Vec<String>,
    default_member: Option<String>,
}

#[derive(Debug, Default)]
struct Manifest {
    package_name: Option<String>,
    workspace: Option<WorkspaceDecl>,
}

// -- manifest parsing (workspace.zig, statement for statement) -------------

fn trim(s: &str) -> &str {
    s.trim_matches(|c| c == ' ' || c == '\t' || c == '\r' || c == '\n')
}

/// Zig `stripInlineComment`: `//` opens a comment when at line
/// start or preceded by whitespace, unless inside a string.
fn strip_inline_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut escape = false;
    let mut i = 0;
    while i + 1 < bytes.len() {
        let c = bytes[i];
        if in_string {
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => in_string = true,
            b'/' => {
                if bytes[i + 1] == b'/' {
                    let prev = if i == 0 { b' ' } else { bytes[i - 1] };
                    if i == 0 || prev.is_ascii_whitespace() {
                        return &line[..i];
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    line
}

fn clean_line(raw: &str) -> &str {
    trim(strip_inline_comment(raw))
}

fn next_token(text: &str, pos: &mut usize) -> Option<String> {
    let bytes = text.as_bytes();
    while *pos < bytes.len() && bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
    if *pos >= bytes.len() {
        return None;
    }
    let start = *pos;
    while *pos < bytes.len() && !bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
    Some(text[start..*pos].to_string())
}

fn parse_text_value(text: &str) -> Result<String, ResolveError> {
    let t = trim(text);
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        return Ok(t[1..t.len() - 1].to_string());
    }
    if t.is_empty() {
        return Err(ResolveError::InvalidFormat);
    }
    Ok(t.to_string())
}

fn parse_unique_string_list_into(text: &str, list: &mut Vec<String>) -> Result<(), ResolveError> {
    let t = trim(text);
    if t.len() < 2 || !t.starts_with('[') || !t.ends_with(']') {
        return Err(ResolveError::InvalidFormat);
    }
    let body = trim(&t[1..t.len() - 1]);
    if body.is_empty() {
        return Ok(());
    }
    let before = list.len();
    for fragment in body.split(',') {
        let token = trim(fragment);
        if token.is_empty() {
            return Err(ResolveError::InvalidFormat);
        }
        list.push(parse_text_value(token)?);
    }
    for i in before..list.len() {
        for (j, other) in list.iter().enumerate() {
            if j != i && list[i] == *other {
                return Err(ResolveError::DuplicateEntry);
            }
        }
    }
    Ok(())
}

fn starts_with_word(text: &str, word: &str) -> bool {
    if !text.starts_with(word) {
        return false;
    }
    if text.len() == word.len() {
        return true;
    }
    let next = text.as_bytes()[word.len()];
    next.is_ascii_whitespace() || next == b'[' || next == b'{'
}

fn parse_manifest(source: &str) -> Result<Manifest, ResolveError> {
    let mut package_name: Option<String> = None;
    let mut members: Vec<String> = Vec::new();
    let mut default_member: Option<String> = None;
    let mut workspace_present = false;
    let mut in_workspace = false;

    for raw_line in source.split('\n') {
        let line = clean_line(raw_line);
        if line.is_empty() {
            continue;
        }
        if in_workspace {
            if line == "}" {
                in_workspace = false;
                continue;
            }
            let mut pos = 0;
            let key = next_token(line, &mut pos).ok_or(ResolveError::InvalidFormat)?;
            let value = trim(&line[pos..]);
            if key == "members" {
                parse_unique_string_list_into(value, &mut members)?;
                continue;
            }
            if key == "default_member" {
                if default_member.is_some() {
                    return Err(ResolveError::DuplicateEntry);
                }
                default_member = Some(parse_text_value(value)?);
                continue;
            }
            return Err(ResolveError::InvalidFormat);
        }
        if starts_with_word(line, "workspace") {
            if workspace_present {
                return Err(ResolveError::DuplicateEntry);
            }
            let mut pos = 0;
            let keyword = next_token(line, &mut pos).ok_or(ResolveError::InvalidFormat)?;
            if keyword != "workspace" || trim(&line[pos..]) != "{" {
                return Err(ResolveError::InvalidFormat);
            }
            workspace_present = true;
            in_workspace = true;
            continue;
        }
        if starts_with_word(line, "package") {
            if package_name.is_some() {
                return Err(ResolveError::DuplicateEntry);
            }
            let mut pos = 0;
            let keyword = next_token(line, &mut pos).ok_or(ResolveError::InvalidFormat)?;
            if keyword != "package" {
                return Err(ResolveError::InvalidFormat);
            }
            package_name = Some(parse_text_value(trim(&line[pos..]))?);
            continue;
        }
        // Any other top-level line is ignored by the Zig parser.
    }
    if in_workspace {
        return Err(ResolveError::InvalidFormat);
    }
    Ok(Manifest {
        package_name,
        workspace: if workspace_present {
            Some(WorkspaceDecl {
                members,
                default_member,
            })
        } else {
            None
        },
    })
}

fn read_manifest_file(path: &Path) -> Result<Option<Manifest>, ResolveError> {
    match std::fs::read_to_string(path) {
        Ok(source) => Ok(Some(parse_manifest(&source)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ResolveError::InvalidPath),
    }
}

// -- path helpers ------------------------------------------------------------

fn realpath(path: &Path) -> Result<PathBuf, ResolveError> {
    std::fs::canonicalize(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ResolveError::FileNotFound,
        _ => ResolveError::InvalidPath,
    })
}

fn realpath_if_exists(path: &Path) -> Result<Option<PathBuf>, ResolveError> {
    match std::fs::canonicalize(path) {
        Ok(p) => Ok(Some(p)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ResolveError::InvalidPath),
    }
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn path_contains(path: &str, prefix: &str) -> bool {
    if path == prefix {
        return true;
    }
    if !path.starts_with(prefix) {
        return false;
    }
    if prefix.is_empty() {
        return true;
    }
    if prefix.ends_with('/') {
        return true;
    }
    path.len() > prefix.len() && path.as_bytes()[prefix.len()] == b'/'
}

// -- member listing / selection ----------------------------------------------

struct WorkspaceMember {
    rel_path: String,
    member_root: String,
    package_name: Option<String>,
}

fn member_root_path(workspace_root: &str, member_rel_path: &str) -> Result<String, ResolveError> {
    let joined = Path::new(workspace_root).join(member_rel_path);
    if let Some(real) = realpath_if_exists(&joined)? {
        return Ok(path_str(&real));
    }
    Ok(path_str(&joined))
}

fn list_workspace_members(
    workspace_root: &str,
    manifest: &Manifest,
) -> Result<Vec<WorkspaceMember>, ResolveError> {
    let mut out = Vec::new();
    if let Some(decl) = &manifest.workspace {
        for rel in &decl.members {
            let member_root = member_root_path(workspace_root, rel)?;
            let manifest_path = Path::new(&member_root).join("sa.mod");
            let member_manifest = read_manifest_file(&manifest_path)?;
            let package_name = member_manifest.as_ref().and_then(|m| m.package_name.clone());
            out.push(WorkspaceMember {
                rel_path: rel.clone(),
                member_root,
                package_name,
            });
        }
    } else {
        out.push(WorkspaceMember {
            rel_path: ".".to_string(),
            member_root: workspace_root.to_string(),
            package_name: manifest.package_name.clone(),
        });
    }
    Ok(out)
}

fn find_member_index_by_path(members: &[WorkspaceMember], path: &str) -> Option<usize> {
    let mut match_index: Option<usize> = None;
    let mut match_len = 0usize;
    for (idx, member) in members.iter().enumerate() {
        if !path_contains(path, &member.member_root) {
            continue;
        }
        if member.member_root.len() < match_len {
            continue;
        }
        match_index = Some(idx);
        match_len = member.member_root.len();
    }
    match_index
}

fn select_workspace_member(
    workspace_root: &str,
    manifest: &Manifest,
    request: Option<&str>,
    preferred_member_path: Option<&str>,
) -> Result<WorkspaceMember, ResolveError> {
    let members = list_workspace_members(workspace_root, manifest)?;
    let requested = request.map(trim);
    let mut selected_index: Option<usize> = None;

    if let Some(req) = requested {
        for (idx, member) in members.iter().enumerate() {
            if member.rel_path == req || member.package_name.as_deref() == Some(req) {
                selected_index = Some(idx);
                break;
            }
        }
        if selected_index.is_none() {
            return Err(ResolveError::UnknownPackage);
        }
    } else if let Some(path) = preferred_member_path {
        selected_index = find_member_index_by_path(&members, path);
    }

    if selected_index.is_none() {
        if let Some(decl) = &manifest.workspace {
            if let Some(default_member) = &decl.default_member {
                for (idx, member) in members.iter().enumerate() {
                    if &member.rel_path == default_member
                        || member.package_name.as_deref() == Some(default_member.as_str())
                    {
                        selected_index = Some(idx);
                        break;
                    }
                }
                if selected_index.is_none() {
                    return Err(ResolveError::MissingDefaultMember);
                }
            } else if members.len() == 1 {
                selected_index = Some(0);
            } else {
                return Err(ResolveError::MissingDefaultMember);
            }
        } else {
            selected_index = Some(0);
        }
    }

    let selected = &members[selected_index.expect("selection resolved above")];
    Ok(WorkspaceMember {
        rel_path: selected.rel_path.clone(),
        member_root: selected.member_root.clone(),
        package_name: selected.package_name.clone(),
    })
}

fn find_nearest_manifest_root(start_path: &Path) -> Result<String, ResolveError> {
    let mut current = realpath(start_path)?;
    let fallback = current.clone();
    loop {
        if current.join("sa.mod").exists() {
            return Ok(path_str(&current));
        }
        let parent = match current.parent() {
            Some(p) => p.to_path_buf(),
            None => break,
        };
        if parent == current {
            break;
        }
        current = parent;
    }
    Ok(path_str(&fallback))
}

fn workspace_contains_path(
    workspace_root: &str,
    manifest: &Manifest,
    candidate_path: &str,
) -> Result<bool, ResolveError> {
    let members = list_workspace_members(workspace_root, manifest)?;
    Ok(find_member_index_by_path(&members, candidate_path).is_some())
}

fn find_ancestor_workspace_root(member_root: &str) -> Result<Option<String>, ResolveError> {
    let mut current = match Path::new(member_root).parent() {
        Some(p) => path_str(p),
        None => return Ok(None),
    };
    loop {
        if current == member_root {
            break;
        }
        let manifest_path = Path::new(&current).join("sa.mod");
        if let Some(manifest) = read_manifest_file(&manifest_path)? {
            if manifest.workspace.is_some()
                && workspace_contains_path(&current, &manifest, member_root)?
            {
                return Ok(Some(current));
            }
        }
        let parent = match Path::new(&current).parent() {
            Some(p) => path_str(p),
            None => break,
        };
        if parent == current {
            break;
        }
        current = parent;
    }
    Ok(None)
}

// -- top-level resolution (resolveFromRootPath / selectedSourcePath) ---------

struct PackageResolution {
    member_root: String,
}

fn resolve_from_root_path(
    root_path: &Path,
    request: Option<&str>,
) -> Result<PackageResolution, ResolveError> {
    let start_real = realpath(root_path)?;
    let anchor_root = find_nearest_manifest_root(&start_real)?;
    let ancestor_workspace_root = find_ancestor_workspace_root(&anchor_root)?;
    let (workspace_root, preferred) = match &ancestor_workspace_root {
        Some(root) => (root.clone(), anchor_root.clone()),
        None => (anchor_root.clone(), path_str(&start_real)),
    };
    let manifest_path = Path::new(&workspace_root).join("sa.mod");
    let manifest = read_manifest_file(&manifest_path)?;
    let member_root = match &manifest {
        Some(m) => {
            select_workspace_member(&workspace_root, m, request, Some(&preferred))?.member_root
        }
        None => workspace_root.clone(),
    };
    Ok(PackageResolution { member_root })
}

fn selected_source_path(resolved: &PackageResolution) -> Result<String, ResolveError> {
    let src = Path::new(&resolved.member_root).join("src").join("main.sla");
    if src.exists() {
        return Ok(path_str(&src));
    }
    let fallback = Path::new(&resolved.member_root).join("main.sla");
    if fallback.exists() {
        return Ok(path_str(&fallback));
    }
    Err(ResolveError::FileNotFound)
}

/// Zig `resolveWorkspaceSourcePath` (plugin_compile.zig), including
/// its printed error lines. Ok(None) = error already reported,
/// caller returns 1; Err(()) = an unmapped Zig error propagated
/// as a hard failure.
pub fn resolve_workspace_source_path(
    stderr: &mut impl Write,
    package_name: Option<&str>,
) -> Result<Option<String>, ()> {
    let resolution = match resolve_from_root_path(Path::new("."), package_name) {
        Ok(r) => r,
        Err(ResolveError::FileNotFound) => {
            write!(stderr, "Error: missing file argument and no workspace source could be resolved from the current directory\n").map_err(|_| ())?;
            return Ok(None);
        }
        Err(ResolveError::UnknownPackage) => {
            writeln!(
                stderr,
                "Error: unknown workspace package: {}",
                package_name.unwrap_or("")
            )
            .map_err(|_| ())?;
            return Ok(None);
        }
        Err(ResolveError::MissingDefaultMember) => {
            write!(stderr, "Error: workspace has no resolvable default member; pass -p/--package or run inside a member directory\n").map_err(|_| ())?;
            return Ok(None);
        }
        Err(ResolveError::InvalidFormat) => {
            write!(stderr, "Error: failed to parse workspace sa.mod\n").map_err(|_| ())?;
            return Ok(None);
        }
        Err(_) => return Err(()),
    };
    match selected_source_path(&resolution) {
        Ok(path) => Ok(Some(path)),
        Err(ResolveError::FileNotFound) => {
            write!(stderr, "Error: workspace member has no src/main.sla or main.sla entry source\n").map_err(|_| ())?;
            Ok(None)
        }
        Err(_) => Err(()),
    }
}

/// Zig `resolveSlaInputFile`: an explicit source file wins verbatim;
/// otherwise the workspace resolution runs.
pub fn resolve_sla_input_file(
    stderr: &mut impl Write,
    source_file: Option<&str>,
    package_name: Option<&str>,
) -> Result<Option<String>, ()> {
    if let Some(file) = source_file {
        return Ok(Some(file.to_string()));
    }
    resolve_workspace_source_path(stderr, package_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> TempDir {
            let n = NEXT.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "sla_cli_workspace_{}_{n}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(std::fs::canonicalize(&dir).unwrap())
        }
        fn write(&self, rel: &str, content: &str) {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn manifest_parse_shapes() {
        let m = parse_manifest("package \"app\"\n").unwrap();
        assert_eq!(m.package_name.as_deref(), Some("app"));
        assert!(m.workspace.is_none());
        let m = parse_manifest(
            "workspace {\n  members [\"members/app\", \"members/tool\"]\n  default_member \"app\"\n}\n",
        )
        .unwrap();
        let ws = m.workspace.unwrap();
        assert_eq!(ws.members.len(), 2);
        assert_eq!(ws.default_member.as_deref(), Some("app"));
        let m = parse_manifest("package app // note\n").unwrap();
        assert_eq!(m.package_name.as_deref(), Some("app"));
        assert_eq!(
            parse_manifest("workspace {\n members [\"a\", \"a\"]\n}\n").err(),
            Some(ResolveError::DuplicateEntry)
        );
        assert_eq!(
            parse_manifest("workspace {\n bogus 1\n}\n").err(),
            Some(ResolveError::InvalidFormat)
        );
    }

    #[test]
    fn resolver_selects_default_member_by_package_name() {
        // The workspace.zig unit test, ported.
        let tmp = TempDir::new();
        tmp.write(
            "sa.mod",
            "workspace {\n  members [\"members/app\", \"members/tool\"]\n  default_member \"app\"\n}\n",
        );
        tmp.write("members/app/sa.mod", "package \"app\"\n");
        tmp.write("members/app/src/main.sla", "fn main() -> i32 {\n    return 0;\n};\n");
        tmp.write("members/tool/sa.mod", "package \"tool\"\n");
        tmp.write("members/tool/src/main.sla", "fn main() -> i32 {\n    return 1;\n};\n");
        let resolved = resolve_from_root_path(&tmp.0, None).unwrap();
        let source = selected_source_path(&resolved).unwrap();
        assert!(source.ends_with("members/app/src/main.sla"), "{source}");
        let resolved = resolve_from_root_path(&tmp.0, Some("members/tool")).unwrap();
        let source = selected_source_path(&resolved).unwrap();
        assert!(source.ends_with("members/tool/src/main.sla"), "{source}");
        assert_eq!(
            resolve_from_root_path(&tmp.0, Some("nope")).err(),
            Some(ResolveError::UnknownPackage)
        );
    }

    #[test]
    fn resolver_single_package_from_member_dir() {
        let tmp = TempDir::new();
        tmp.write("sa.mod", "package \"app\"\n");
        tmp.write("src/main.sla", "fn main() -> i32 {\n    return 0;\n};\n");
        let member_src = tmp.0.join("src");
        let resolved = resolve_from_root_path(&member_src, None).unwrap();
        let source = selected_source_path(&resolved).unwrap();
        assert!(source.ends_with("src/main.sla"), "{source}");
    }
}
