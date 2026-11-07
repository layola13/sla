//! Command-layer IO abstraction for the metadata commands
//! (skills / stability), ported from sla-plugin's cmdio module so
//! the A0-proven command implementations run unchanged against
//! the standalone binary's real streams or a test buffer. The
//! Zig error-name table itself lives in `crate::out` and is
//! re-exported here under the name the ported code expects.

pub use crate::out::zig_error_name;

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

/// Adapter over one of the binary's real streams.
pub struct IoSink<'a>(pub &'a mut dyn std::io::Write);

impl CmdWrite for IoSink<'_> {
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ()> {
        std::io::Write::write_all(self.0, bytes).map_err(|_| ())
    }
}

/// Zig `writeCommandHelp`, rendered through cli_help's exact
/// byte path into a buffer first (the Vec sink cannot fail).
pub fn write_command_help(w: &mut impl CmdWrite, command: &str) -> Result<(), ()> {
    let mut buf: Vec<u8> = Vec::new();
    crate::cli_help::write_command_help(&mut buf, command).map_err(|_| ())?;
    w.write_all(&buf)
}
