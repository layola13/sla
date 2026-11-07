//! sla — the standalone SLA compiler binary.
//!
//! Phase-4 delivery form (user ruling 2026-11-07): the SLA
//! command layer as its own executable instead of a host-loaded
//! plugin cdylib. The command surface mirrors the Zig plugin's
//! `runSlaCommandImpl` (plugin_commands.zig) command for command:
//! the front-end pipeline (parse / contract load / type check /
//! text + SAB codegen) runs in-process through the workspace's
//! pipeline crates, and the final compile step delegates to the
//! installed `sa` toolchain's command line — the same seam the
//! Zig plugin itself uses (`sa build-exe`, `sa build-wasm`,
//! `sa build-js`, `sa build-c`, `sa test` over a managed SAB
//! artifact; see delegate.rs and the delivery report's coupling
//! analysis).
//!
//! Usage: `sla <command> [options]` where <command> is one of
//! help / init / build / build-workspace / build-exe / build-wasm
//! / build-js / build-c / sab build|workspace|disasm / slab ... /
//! check / test. Exit codes are the per-command codes the Zig
//! original returns.

mod cli_help;
mod cmdio;
mod commands;
mod delegate;
mod freshness;
mod init_cmd;
mod options;
mod out;
mod pipeline;
mod sab_paths;
mod skills_cmd;
mod stability;
mod stability_validate;
mod workspace;
mod wyhash;

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    // argv as raw bytes, minus the binary name: the Zig command
    // layer is byte-oriented and file names need not be UTF-8.
    let args: Vec<Vec<u8>> = std::env::args_os()
        .skip(1)
        .map(|a| {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                a.as_os_str().as_bytes().to_vec()
            }
            #[cfg(not(unix))]
            {
                a.to_string_lossy().into_owned().into_bytes()
            }
        })
        .collect();
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    let code = commands::dispatch(&args, &mut stdout, &mut stderr);
    let _ = stdout.flush();
    let _ = stderr.flush();
    ExitCode::from(code)
}
