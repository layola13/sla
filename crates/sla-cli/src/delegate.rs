//! Host delegation — mirror of `host_paths.zig`
//! `resolveSaExecutable` and the child-process tail of the
//! delegated commands in plugin_commands.zig.
//!
//! COUPLING SURFACE (the phase-4 foundation decision; full
//! evidence in the delivery REPORT): the final compile step is
//! NOT linked in. This binary shells out to the installed `sa`
//! executable exactly where the Zig plugin does:
//!   build-exe      -> `sa build-exe  <managed.sab> [extra...]`
//!   build-workspace-> `sa build-exe  <managed.sab> [extra...]`
//!   sab workspace  -> `sa build-exe  <managed.sab> [passthrough]`
//!   build-wasm     -> `sa build-wasm <managed.sab> [extra...]`
//!   build-js       -> `sa build-js   <managed.sab> [extra...]`
//!   build-c        -> `sa build-c    <managed.sab> [extra...]`
//!   test           -> `sa test       <compiled input> [passthrough]`
//! The child inherits this process's stdio (Zig `sio.Child`
//! default), its exit code becomes the command's code, and any
//! non-exit termination maps to 1. `appendDefaultJobsAuto` in the
//! original is a deliberate no-op, so argv is the passthrough
//! verbatim.

use crate::out::zig_error_name;
use std::io::Write;

/// Zig `resolveSaExecutable` search order: SA_EXE env (if it
/// names an existing file), SCI_ROOT/zig-out/bin/sa, the
/// directory of the running executable, then each PATH entry;
/// bare "sa" as the final fallback.
pub fn resolve_sa_executable() -> String {
    if let Ok(override_path) = std::env::var("SA_EXE") {
        if !override_path.is_empty() && std::path::Path::new(&override_path).exists() {
            return override_path;
        }
    }
    if let Ok(sci_root) = std::env::var("SCI_ROOT") {
        let candidate = std::path::Path::new(&sci_root)
            .join("zig-out")
            .join("bin")
            .join("sa");
        if candidate.exists() {
            return candidate.to_string_lossy().into_owned();
        }
    }
    if let Ok(self_path) = std::env::current_exe() {
        if let Some(self_dir) = self_path.parent() {
            let candidate = self_dir.join("sa");
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    if let Ok(path_val) = std::env::var("PATH") {
        for entry in path_val.split(|c| c == ':' || c == ';' || c == '\n') {
            if entry.is_empty() {
                continue;
            }
            let candidate = std::path::Path::new(entry).join("sa");
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    "sa".to_string()
}

/// Spawn `argv` with inherited stdio and wait, mirroring
/// `sio.Child.spawnAndWait`. On a spawn failure the Zig commands
/// print `Error: failed to run '<what>': error.{name}` and return
/// 1 — `what` is the caller's literal (e.g. `'sa build-exe'`,
/// `'sa build-exe' for SAB workspace output`, `'sa test'`).
pub fn run_child(argv: &[String], what: &str, stderr: &mut impl Write) -> u8 {
    let (exe, rest) = argv.split_first().expect("delegated argv is non-empty");
    match std::process::Command::new(exe).args(rest).status() {
        Ok(status) => match status.code() {
            Some(code) => (code & 0xff) as u8,
            None => 1,
        },
        Err(err) => {
            let _ = writeln!(
                stderr,
                "Error: failed to run {what}: error.{}",
                zig_error_name(&err)
            );
            1
        }
    }
}

/// The delegated argv: [sa_exe, subcommand, input, extra...].
pub fn delegated_argv(subcommand: &str, input: &str, extra: &[String]) -> Vec<String> {
    let mut argv = vec![
        resolve_sa_executable(),
        subcommand.to_string(),
        input.to_string(),
    ];
    argv.extend_from_slice(extra);
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sa_executable_resolution_shape() {
        let exe = resolve_sa_executable();
        assert!(!exe.is_empty());
    }

    #[test]
    fn delegated_argv_layout() {
        let argv = delegated_argv("build-exe", "x.sab", &["-o".to_string(), "app".to_string()]);
        assert_eq!(argv.len(), 5);
        assert_eq!(argv[1], "build-exe");
        assert_eq!(argv[2], "x.sab");
        assert_eq!(argv[3], "-o");
    }
}
