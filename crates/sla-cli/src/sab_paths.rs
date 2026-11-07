//! SAB output paths — mirror of `plugin_sab_paths.zig` (venue head
//! 2cc12ea8): the managed cache path (Wyhash-named), sibling output
//! paths, `--out`/`--sab-out` argument scans, and the SAB file
//! writer with its exact stderr lines. The std-root / project-root
//! helpers of that file serve `sab_codegen`'s surface loader, which
//! lives in sla-codegen-sab on this side, and are not duplicated
//! here.

use crate::options::sa_test_filter_from_args;
use crate::out::zig_error_name;
use crate::wyhash::{wyhash, WyhashStream};
use std::io::Write;

/// Zig `defaultOutputPath`: swap a trailing extension, else append.
pub fn default_output_path(file: &str, from_ext: &str, to_ext: &str) -> String {
    if let Some(stem) = file.strip_suffix(from_ext) {
        format!("{stem}{to_ext}")
    } else {
        format!("{file}{to_ext}")
    }
}

fn basename(path: &str) -> &str {
    match path.rfind('/') {
        Some(idx) => &path[idx + 1..],
        None => path,
    }
}

/// Zig `managedSabPath`: `.sla-cache/sab/{stem}-{hash:x}.sab` where
/// the hash is Wyhash(seed 0) over the file path AS GIVEN (not
/// canonicalized) and `{x}` is lowercase hex without padding.
pub fn managed_sab_path(file: &str) -> String {
    let base = basename(file);
    let stem = base.strip_suffix(".sla").unwrap_or(base);
    let hash = wyhash(0, file.as_bytes());
    format!(".sla-cache/sab/{stem}-{hash:x}.sab")
}

fn managed_sab_path_with_variant_parts(
    file: &str,
    variant_name: &str,
    variant_value: Option<&str>,
) -> String {
    let base = basename(file);
    let stem = base.strip_suffix(".sla").unwrap_or(base);
    let mut hasher = WyhashStream::init(0);
    hasher.update(file.as_bytes());
    hasher.update(b"\x00");
    hasher.update(variant_name.as_bytes());
    if let Some(value) = variant_value {
        hasher.update(b"\x00");
        hasher.update(value.as_bytes());
    }
    let hash = hasher.finish();
    format!(".sla-cache/sab/{stem}-{hash:x}.sab")
}

/// Zig `managedSabTestPath`: filter-scoped when a `--filter` is
/// present among the test passthrough args, else the `test-all`
/// variant.
pub fn managed_sab_test_path(file: &str, extra_args: &[Vec<u8>]) -> String {
    if let Some(filter) = sa_test_filter_from_args(extra_args) {
        return managed_sab_path_with_variant_parts(file, "test-filter", Some(&filter));
    }
    managed_sab_path_with_variant_parts(file, "test-all", None)
}

/// Zig `writeSabFile`: ensure the parent directory, skip the write
/// when the existing bytes are already identical, else write.
/// Returns false (after printing the exact Zig error line) on
/// failure. Err(()) is a stderr write failure.
pub fn write_sab_file(
    path: &str,
    sab_bytes: &[u8],
    stderr: &mut impl Write,
) -> Result<bool, ()> {
    if let Some(idx) = path.rfind('/') {
        let dir = &path[..idx];
        if !dir.is_empty() {
            if let Err(err) = std::fs::create_dir_all(dir) {
                writeln!(
                    stderr,
                    "File Error: failed to create SAB output directory {dir}: error.{}",
                    zig_error_name(&err)
                )
                .map_err(|_| ())?;
                return Ok(false);
            }
        }
    }
    if let Ok(existing) = std::fs::read(path) {
        if existing == sab_bytes {
            return Ok(true);
        }
    }
    match std::fs::write(path, sab_bytes) {
        Ok(()) => Ok(true),
        Err(err) => {
            writeln!(
                stderr,
                "File Error: failed to write SAB output {path}: error.{}",
                zig_error_name(&err)
            )
            .map_err(|_| ())?;
            Ok(false)
        }
    }
}

/// Zig `writeManagedSab`: write at the managed path and return it.
pub fn write_managed_sab(
    file: &str,
    sab_bytes: &[u8],
    stderr: &mut impl Write,
) -> Result<Option<String>, ()> {
    let path = managed_sab_path(file);
    if write_sab_file(&path, sab_bytes, stderr)? {
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

/// Zig `parseOutFileArg`: first `--out`/`-o` value at or after
/// `start_idx`; a trailing flag without a value yields None.
pub fn parse_out_file_arg(args: &[Vec<u8>], start_idx: usize) -> Option<String> {
    let mut idx = start_idx;
    while idx < args.len() {
        if args[idx] == b"--out" || args[idx] == b"-o" {
            if idx + 1 < args.len() {
                return Some(crate::out::lossy(&args[idx + 1]));
            }
            return None;
        }
        idx += 1;
    }
    None
}

/// Zig `parseSabOutFileArg`: first `--sab-out` value.
pub fn parse_sab_out_file_arg(args: &[Vec<u8>], start_idx: usize) -> Option<String> {
    let mut idx = start_idx;
    while idx < args.len() {
        if args[idx] == b"--sab-out" {
            if idx + 1 < args.len() {
                return Some(crate::out::lossy(&args[idx + 1]));
            }
            return None;
        }
        idx += 1;
    }
    None
}

/// Zig `hasEmitSabArg`.
pub fn has_emit_sab_arg(args: &[Vec<u8>], start_idx: usize) -> bool {
    args[start_idx.min(args.len())..]
        .iter()
        .any(|arg| arg == b"--emit-sab" || arg == b"--emit-sab-file")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_path_shape() {
        // Shape pinned; the hash value itself is wyhash(0, file),
        // cross-checked live against the Zig plugin's printed
        // managed path in the phase-4 parity matrix.
        let p = managed_sab_path("src/main.sla");
        assert!(p.starts_with(".sla-cache/sab/main-"));
        assert!(p.ends_with(".sab"));
        assert_ne!(managed_sab_path("a/b/c.sla"), managed_sab_path("a/b/d.sla"));
        assert_ne!(managed_sab_test_path("x.sla", &[]), managed_sab_path("x.sla"));
        let filtered: Vec<Vec<u8>> = vec![b"--filter".to_vec(), b"keep".to_vec()];
        let eq_filtered: Vec<Vec<u8>> = vec![b"--filter=keep".to_vec()];
        assert_eq!(
            managed_sab_test_path("x.sla", &filtered),
            managed_sab_test_path("x.sla", &eq_filtered)
        );
    }

    #[test]
    fn default_output_path_swaps_extension() {
        assert_eq!(default_output_path("a/b.sla", ".sla", ".sab"), "a/b.sab");
        assert_eq!(default_output_path("a/b", ".sla", ".sab"), "a/b.sab");
    }

    #[test]
    fn out_arg_scans() {
        let args: Vec<Vec<u8>> = vec![
            b"sab".to_vec(),
            b"build".to_vec(),
            b"--out".to_vec(),
            b"o.sab".to_vec(),
        ];
        assert_eq!(parse_out_file_arg(&args, 2).as_deref(), Some("o.sab"));
        assert_eq!(parse_out_file_arg(&args, 3), None);
        assert_eq!(parse_sab_out_file_arg(&args, 0), None);
    }
}
