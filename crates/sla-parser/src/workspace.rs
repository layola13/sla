//! Workspace / `sa.mod` manifest machinery, mirrored from
//! `sa_plugin_sla/src/workspace.zig` (705 lines at the box head
//! 2cc12ea8).
//!
//! Mirrored surface: manifest parsing (`package` / `workspace {`
//! blocks with `members` / `default_member`), member listing,
//! member selection (explicit request -> preferred path -> default
//! member -> single member), nearest-manifest and ancestor-workspace
//! root discovery, and the selected member's source path. Zig's
//! `ResolveError.OutOfMemory` has no Rust analogue and is dropped;
//! every other error kind is carried verbatim.
//!
//! Quirks mirrored on purpose:
//!   * lines outside the `workspace { ... }` block that are neither
//!     `package` nor `workspace` are silently IGNORED (no error);
//!   * inside the block, an unknown key is InvalidFormat;
//!   * `//` comments are stripped only when the `//` is at line
//!     start or preceded by whitespace, and never inside strings;
//!   * member-by-path matching keeps the LONGEST matching member
//!     root, with later members winning exact-length ties
//!     (Zig's `match_len` starts at 0 and `<` is the only skip).

use std::path::{Path, PathBuf};

/// Zig `workspace.ResolveError` (minus OutOfMemory).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceError {
    FileNotFound,
    InvalidFormat,
    InvalidPath,
    DuplicateEntry,
    UnknownPackage,
    MissingDefaultMember,
}

impl std::fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            WorkspaceError::FileNotFound => "FileNotFound",
            WorkspaceError::InvalidFormat => "InvalidFormat",
            WorkspaceError::InvalidPath => "InvalidPath",
            WorkspaceError::DuplicateEntry => "DuplicateEntry",
            WorkspaceError::UnknownPackage => "UnknownPackage",
            WorkspaceError::MissingDefaultMember => "MissingDefaultMember",
        };
        write!(f, "error.{name}")
    }
}

impl std::error::Error for WorkspaceError {}

type Result<T> = std::result::Result<T, WorkspaceError>;
use WorkspaceError::*;

/// Zig `WorkspaceDecl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDecl {
    pub members: Vec<String>,
    pub default_member: Option<String>,
}

/// Zig `Manifest`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Manifest {
    pub package_name: Option<String>,
    pub workspace: Option<WorkspaceDecl>,
}

/// Zig `PackageResolution`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageResolution {
    pub workspace_root: String,
    pub member_root: String,
    pub workspace_manifest_path: String,
    pub member_manifest_path: String,
    pub workspace_manifest: Option<Manifest>,
    pub member_manifest: Option<Manifest>,
    pub selected_package: Option<String>,
    pub workspace_rel_member_path: Option<String>,
}

/// Zig `WorkspaceMember`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMember {
    pub rel_path: String,
    pub member_root: String,
    pub package_name: Option<String>,
}

// -- text helpers (Zig private fns, same order) ---------------------

fn trim(text: &str) -> &str {
    text.trim_matches(|c| c == ' ' || c == '\t' || c == '\r' || c == '\n')
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

/// Zig `stripInlineComment`.
fn strip_inline_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut escape = false;
    let mut i = 0usize;
    while i + 1 < line.len() {
        let c = bytes[i];
        if in_string {
            if escape {
                escape = false;
            } else {
                match c {
                    b'\\' => escape = true,
                    b'"' => in_string = false,
                    _ => {}
                }
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

/// Zig `nextToken`: returns (token, rest-position).
fn next_token<'a>(text: &'a str, pos: &mut usize) -> Option<&'a str> {
    let bytes = text.as_bytes();
    while *pos < text.len() && bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
    if *pos >= text.len() {
        return None;
    }
    let start = *pos;
    while *pos < text.len() && !bytes[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
    Some(&text[start..*pos])
}

/// Zig `parseTextValue`.
fn parse_text_value(text: &str) -> Result<String> {
    let trimmed = trim(text);
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        return Ok(trimmed[1..trimmed.len() - 1].to_string());
    }
    if trimmed.is_empty() {
        return Err(InvalidFormat);
    }
    Ok(trimmed.to_string())
}

fn string_list_contains(items: &[String], needle: &str) -> bool {
    items.iter().any(|i| i == needle)
}

/// Zig `parseStringListInto`.
fn parse_string_list_into(text: &str, list: &mut Vec<String>) -> Result<()> {
    let trimmed = trim(text);
    if trimmed.len() < 2 || !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err(InvalidFormat);
    }
    let body = trim(&trimmed[1..trimmed.len() - 1]);
    if body.is_empty() {
        return Ok(());
    }
    for fragment in body.split(',') {
        let token = trim(fragment);
        if token.is_empty() {
            return Err(InvalidFormat);
        }
        list.push(parse_text_value(token)?);
    }
    Ok(())
}

/// Zig `parseUniqueStringListInto`.
fn parse_unique_string_list_into(text: &str, list: &mut Vec<String>) -> Result<()> {
    let before_len = list.len();
    if let Err(e) = parse_string_list_into(text, list) {
        list.truncate(before_len);
        return Err(e);
    }
    for item in &list[before_len..] {
        if string_list_contains(&list[..before_len], item) {
            return Err(DuplicateEntry);
        }
    }
    let mut i = before_len;
    while i < list.len() {
        let mut j = i + 1;
        while j < list.len() {
            if list[i] == list[j] {
                return Err(DuplicateEntry);
            }
            j += 1;
        }
        i += 1;
    }
    Ok(())
}

/// Zig `parsePackageDecl`.
fn parse_package_decl(line: &str) -> Result<String> {
    let mut pos = 0usize;
    let keyword = next_token(line, &mut pos).ok_or(InvalidFormat)?;
    if keyword != "package" {
        return Err(InvalidFormat);
    }
    parse_text_value(trim(&line[pos..]))
}

/// Zig `parseWorkspaceHeader`.
fn parse_workspace_header(line: &str) -> Result<()> {
    let mut pos = 0usize;
    let keyword = next_token(line, &mut pos).ok_or(InvalidFormat)?;
    if keyword != "workspace" {
        return Err(InvalidFormat);
    }
    if trim(&line[pos..]) != "{" {
        return Err(InvalidFormat);
    }
    Ok(())
}

/// Zig `parseManifestWithFile`. Exposed publicly so the plugin
/// wiring and tests can pin manifest verdicts without a filesystem
/// (Zig keeps it private; the exposure changes no verdict).
pub fn parse_manifest(source: &str) -> Result<Manifest> {
    let mut package_name: Option<String> = None;
    let mut workspace_members: Vec<String> = Vec::new();
    let mut workspace_default_member: Option<String> = None;
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
            let mut pos = 0usize;
            let key = next_token(line, &mut pos).ok_or(InvalidFormat)?;
            let value = trim(&line[pos..]);
            if key == "members" {
                parse_unique_string_list_into(value, &mut workspace_members)?;
                continue;
            }
            if key == "default_member" {
                if workspace_default_member.is_some() {
                    return Err(DuplicateEntry);
                }
                workspace_default_member = Some(parse_text_value(value)?);
                continue;
            }
            return Err(InvalidFormat);
        }
        if starts_with_word(line, "workspace") {
            if workspace_present {
                return Err(DuplicateEntry);
            }
            parse_workspace_header(line)?;
            workspace_present = true;
            in_workspace = true;
            continue;
        }
        if starts_with_word(line, "package") {
            if package_name.is_some() {
                return Err(DuplicateEntry);
            }
            package_name = Some(parse_package_decl(line)?);
            continue;
        }
        // Zig: any other top-level line is ignored.
    }
    if in_workspace {
        return Err(InvalidFormat);
    }
    Ok(Manifest {
        package_name,
        workspace: if workspace_present {
            Some(WorkspaceDecl {
                members: workspace_members,
                default_member: workspace_default_member,
            })
        } else {
            None
        },
    })
}

// -- filesystem helpers ----------------------------------------------

fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

fn path_join(parts: &[&str]) -> String {
    let mut buf = PathBuf::new();
    for part in parts {
        buf.push(part);
    }
    normalize_slashes(&buf.to_string_lossy())
}

/// Zig `realpathAllocResolved`.
fn realpath_resolved(path: &str) -> Result<String> {
    match std::fs::canonicalize(path) {
        Ok(p) => Ok(normalize_slashes(&p.to_string_lossy())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(FileNotFound),
        Err(_) => Err(InvalidPath),
    }
}

/// Zig `realpathAllocIfExists`.
fn realpath_if_exists(path: &str) -> Result<Option<String>> {
    match std::fs::canonicalize(path) {
        Ok(p) => Ok(Some(normalize_slashes(&p.to_string_lossy()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(InvalidPath),
    }
}

fn path_exists(path: &str) -> bool {
    Path::new(path).exists()
}

/// Zig `readManifestFile`: missing -> None; unreadable -> InvalidPath;
/// parse errors propagate as InvalidFormat / DuplicateEntry.
fn read_manifest_file(path: &str) -> Result<Option<Manifest>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(InvalidPath),
    };
    let source = String::from_utf8_lossy(&bytes);
    match parse_manifest(&source) {
        Ok(m) => Ok(Some(m)),
        Err(e @ (InvalidFormat | DuplicateEntry)) => Err(e),
        Err(e) => Err(e),
    }
}

fn manifest_path_for_root(root_path: &str) -> String {
    path_join(&[root_path, "sa.mod"])
}

/// Zig `sourcePathForRoot`.
fn source_path_for_root(root_path: &str) -> Result<String> {
    let src_path = path_join(&[root_path, "src", "main.sla"]);
    if path_exists(&src_path) {
        return Ok(src_path);
    }
    let fallback = path_join(&[root_path, "main.sla"]);
    if path_exists(&fallback) {
        return Ok(fallback);
    }
    Err(FileNotFound)
}

fn dirname(path: &str) -> Option<String> {
    Path::new(path)
        .parent()
        .map(|p| normalize_slashes(&p.to_string_lossy()))
}

/// Zig `pathContains`.
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

/// Zig `memberRootPath`.
fn member_root_path(workspace_root: &str, member_rel_path: &str) -> Result<String> {
    let joined = path_join(&[workspace_root, member_rel_path]);
    if let Some(real) = realpath_if_exists(&joined)? {
        return Ok(real);
    }
    Ok(joined)
}

/// Zig `findNearestManifestRoot`.
fn find_nearest_manifest_root(start_path: &str) -> Result<String> {
    let mut current = realpath_resolved(start_path)?;
    let fallback = current.clone();
    loop {
        let manifest_path = manifest_path_for_root(&current);
        if path_exists(&manifest_path) {
            return Ok(current);
        }
        let parent = match dirname(&current) {
            Some(p) => p,
            None => break,
        };
        if parent == current {
            break;
        }
        current = parent;
    }
    Ok(fallback)
}

/// Zig `listWorkspaceMembers`.
pub fn list_workspace_members(
    workspace_root: &str,
    workspace_manifest: &Manifest,
) -> Result<Vec<WorkspaceMember>> {
    let mut members = Vec::new();
    if let Some(decl) = &workspace_manifest.workspace {
        for member_rel_path in &decl.members {
            let member_root = member_root_path(workspace_root, member_rel_path)?;
            let member_manifest_path = path_join(&[&member_root, "sa.mod"]);
            let maybe_member_manifest = read_manifest_file(&member_manifest_path)?;
            let package_name = maybe_member_manifest
                .as_ref()
                .and_then(|m| m.package_name.clone());
            members.push(WorkspaceMember {
                rel_path: member_rel_path.clone(),
                member_root,
                package_name,
            });
        }
    } else {
        members.push(WorkspaceMember {
            rel_path: ".".to_string(),
            member_root: workspace_root.to_string(),
            package_name: workspace_manifest.package_name.clone(),
        });
    }
    Ok(members)
}

/// Zig `findMemberIndexByPath`.
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

struct SelectedMember {
    member_root: String,
    rel_path: String,
    package_name: Option<String>,
}

/// Zig `selectWorkspaceMember`.
fn select_workspace_member(
    workspace_root: &str,
    workspace_manifest: &Manifest,
    selection: Option<&str>,
    preferred_member_path: Option<&str>,
) -> Result<SelectedMember> {
    let members = list_workspace_members(workspace_root, workspace_manifest)?;
    let requested = selection.map(trim);
    let workspace_decl = &workspace_manifest.workspace;

    let mut selected_index: Option<usize> = None;
    if let Some(req) = requested {
        for (idx, member) in members.iter().enumerate() {
            if member.rel_path == req {
                selected_index = Some(idx);
                break;
            }
            if let Some(package_name) = &member.package_name {
                if package_name == req {
                    selected_index = Some(idx);
                    break;
                }
            }
        }
        if selected_index.is_none() {
            return Err(UnknownPackage);
        }
    } else if let Some(path) = preferred_member_path {
        selected_index = find_member_index_by_path(&members, path);
    }

    if selected_index.is_none() {
        if let Some(decl) = workspace_decl {
            if let Some(default_member) = &decl.default_member {
                for (idx, member) in members.iter().enumerate() {
                    if &member.rel_path == default_member {
                        selected_index = Some(idx);
                        break;
                    }
                    if let Some(package_name) = &member.package_name {
                        if package_name == default_member {
                            selected_index = Some(idx);
                            break;
                        }
                    }
                }
                if selected_index.is_none() {
                    return Err(MissingDefaultMember);
                }
            } else if members.len() == 1 {
                selected_index = Some(0);
            } else {
                return Err(MissingDefaultMember);
            }
        } else {
            selected_index = Some(0);
        }
    }

    let selected = &members[selected_index.unwrap()];
    Ok(SelectedMember {
        member_root: selected.member_root.clone(),
        rel_path: selected.rel_path.clone(),
        package_name: selected.package_name.clone(),
    })
}

/// Zig `workspaceContainsPath`.
fn workspace_contains_path(
    workspace_root: &str,
    workspace_manifest: &Manifest,
    candidate_path: &str,
) -> Result<bool> {
    let members = list_workspace_members(workspace_root, workspace_manifest)?;
    Ok(find_member_index_by_path(&members, candidate_path).is_some())
}

/// Zig `findAncestorWorkspaceRoot`.
fn find_ancestor_workspace_root(member_root: &str) -> Result<Option<String>> {
    let mut current = match dirname(member_root) {
        Some(d) => d,
        None => return Ok(None),
    };
    loop {
        if current == member_root {
            break;
        }
        let manifest_path = manifest_path_for_root(&current);
        let maybe_manifest = read_manifest_file(&manifest_path)?;
        if let Some(workspace_manifest) = maybe_manifest {
            if workspace_manifest.workspace.is_some()
                && workspace_contains_path(&current, &workspace_manifest, member_root)?
            {
                return Ok(Some(current));
            }
        }
        let parent = match dirname(&current) {
            Some(p) => p,
            None => break,
        };
        if parent == current {
            break;
        }
        current = parent;
    }
    Ok(None)
}

/// Zig `resolveFromCurrentDir`.
pub fn resolve_from_current_dir(selection: Option<&str>) -> Result<PackageResolution> {
    resolve_from_root_path(".", selection)
}

/// Zig `resolveFromRootPath`.
pub fn resolve_from_root_path(
    root_path: &str,
    selection: Option<&str>,
) -> Result<PackageResolution> {
    let start_real = realpath_resolved(root_path)?;
    let anchor_root = find_nearest_manifest_root(&start_real)?;
    let ancestor_workspace_root = find_ancestor_workspace_root(&anchor_root)?;
    let workspace_root = ancestor_workspace_root.clone().unwrap_or_else(|| anchor_root.clone());

    let workspace_manifest_path = manifest_path_for_root(&workspace_root);
    let workspace_manifest = read_manifest_file(&workspace_manifest_path)?;

    let selected: SelectedMember = if let Some(ws_manifest) = &workspace_manifest {
        let preferred = if ancestor_workspace_root.is_some() {
            anchor_root.as_str()
        } else {
            start_real.as_str()
        };
        select_workspace_member(&workspace_root, ws_manifest, selection, Some(preferred))?
    } else {
        SelectedMember {
            member_root: workspace_root.clone(),
            rel_path: ".".to_string(),
            package_name: None,
        }
    };

    let member_manifest_path = manifest_path_for_root(&selected.member_root);
    let member_manifest = read_manifest_file(&member_manifest_path)?;

    Ok(PackageResolution {
        workspace_root,
        member_root: selected.member_root,
        workspace_manifest_path,
        member_manifest_path,
        workspace_manifest,
        member_manifest,
        selected_package: selected.package_name,
        workspace_rel_member_path: Some(selected.rel_path),
    })
}

/// Zig `selectedSourcePath`.
pub fn selected_source_path(resolved: &PackageResolution) -> Result<String> {
    source_path_for_root(&resolved.member_root).map(|p| normalize_slashes(&p))
}

#[cfg(test)]
mod tests;
