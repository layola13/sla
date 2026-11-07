//! Byte-sink plumbing for the A0 command surface.
//!
//! Zig's command implementations write through `*std.Io.Writer`
//! over a HostStream sink; a write failure becomes an error that
//! the ABI shell maps to `AbiStatus::Failed`. Here commands are
//! generic over [`CmdWrite`] so the same code runs against the
//! host's streams (in `lib.rs`) and against `Vec<u8>` capture
//! buffers in unit tests. `Err(())` is the write-failure channel
//! and always means "the whole invocation fails", never a command
//! return code.

use sla_abi::HostStream;

pub trait CmdWrite {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()>;

    fn write_str(&mut self, s: &str) -> Result<(), ()> {
        self.write_all(s.as_bytes())
    }

    /// `writeAll` of a `format!` product (Zig `writer.print`).
    fn print(&mut self, args: std::fmt::Arguments<'_>) -> Result<(), ()> {
        let text = std::fmt::format(args);
        self.write_all(text.as_bytes())
    }
}

impl CmdWrite for Vec<u8> {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

/// Adapter over a host stream. The host callback itself reports
/// failure; the ABI shell's contract turns that into Failed.
pub struct HostSink(pub HostStream);

impl CmdWrite for HostSink {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
        // SAFETY: the stream was handed in by the host for the
        // duration of this handle_command call.
        unsafe { self.0.write_all(bytes) }.map_err(|_| ())
    }
}

/// Map a Rust IO error onto the Zig error NAME the original prints
/// with `{}` (error union formatting). Best-effort table for the
/// kinds the A0 surface can produce; unknown kinds fall back to
/// "Unexpected", Zig's own catch-all error name.
pub fn zig_error_name(err: &std::io::Error) -> &'static str {
    use std::io::ErrorKind::*;
    match err.kind() {
        NotFound => "FileNotFound",
        AlreadyExists => "PathAlreadyExists",
        PermissionDenied => "AccessDenied",
        IsADirectory => "IsDir",
        NotADirectory => "NotDir",
        InvalidInput => "InvalidArgument",
        InvalidFilename => "InvalidArgument",
        _ => "Unexpected",
    }
}

/// std.fs.path.join, mirrored for the path shapes the A0 surface
/// joins (init scaffolding): `.` segments drop out and `..`
/// collapses lexically against prior segments — EXCEPT a leading
/// `.` segment in the first part is preserved as a `./` prefix
/// (venue-proven: join(".", "src", "main.sla") == "./src/main.sla",
/// while join("proj", "src", "main.sla") == "proj/src/main.sla").
/// A fully-empty result is "." (Zig join of an empty relative path
/// set).
pub fn zig_join(parts: &[&str]) -> String {
    let mut segs: Vec<&str> = Vec::new();
    let absolute = parts.first().map(|p| p.starts_with('/')).unwrap_or(false);
    let leading_dot = parts
        .first()
        .map(|p| p.split('/').next() == Some("."))
        .unwrap_or(false);
    for part in parts {
        for seg in part.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    if segs.last().map(|s| *s != "..").unwrap_or(false) && !segs.is_empty() {
                        segs.pop();
                    } else if !absolute {
                        segs.push("..");
                    }
                }
                s => segs.push(s),
            }
        }
    }
    if segs.is_empty() {
        return if absolute { "/".to_string() } else { ".".to_string() };
    }
    let mut out = if absolute {
        "/".to_string()
    } else if leading_dot {
        "./".to_string()
    } else {
        String::new()
    };
    out.push_str(&segs.join("/"));
    out
}

/// Zig `std.fs.path.basename`: the final segment; a trailing slash
/// is ignored, and the basename of "/" is "/".
pub fn zig_basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.is_empty() { "" } else { "/" };
    }
    match trimmed.rfind('/') {
        Some(idx) => &trimmed[idx + 1..],
        None => trimmed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_drops_dot_segments_like_zig() {
        // The leading "." of the first part survives as "./" —
        // the rest drop (venue-proven against the Zig plugin).
        assert_eq!(zig_join(&[".", "src", "main.sla"]), "./src/main.sla");
        assert_eq!(zig_join(&[".", "sa.mod"]), "./sa.mod");
        assert_eq!(zig_join(&["proj", "src"]), "proj/src");
        assert_eq!(zig_join(&["proj/", "src"]), "proj/src");
        assert_eq!(zig_join(&["a", "..", "b"]), "b");
        assert_eq!(zig_join(&["."]), ".");
    }

    #[test]
    fn basename_shapes() {
        assert_eq!(zig_basename("proj"), "proj");
        assert_eq!(zig_basename("a/b"), "b");
        assert_eq!(zig_basename("proj/"), "proj");
        assert_eq!(zig_basename("."), ".");
        assert_eq!(zig_basename(""), "");
    }
}
