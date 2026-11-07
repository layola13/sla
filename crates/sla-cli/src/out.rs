//! Small IO helpers shared by the command layer: the Zig error
//! name table (Zig prints errors as `error.{Name}` in the command
//! surface's `{}` positions) and lossy argv rendering.

/// Map a Rust IO error onto the Zig error NAME the original prints
/// (callers add the `error.` prefix Zig's `{}` formatting carries).
/// Same table as the A0 slice in sla-plugin; unknown kinds fall
/// back to "Unexpected", Zig's catch-all error name.
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

/// argv elements are bytes; Zig treats them as byte strings.
pub fn lossy(arg: &[u8]) -> String {
    String::from_utf8_lossy(arg).into_owned()
}
