//! Command implementations — mirror of `runSlaCommandImpl` and
//! its subcommand functions in plugin_commands.zig (venue head
//! 2cc12ea8). Every command returns the process exit code the Zig
//! original returns; a `None` from an Option-returning helper
//! means the diagnostic was already printed, exactly like the
//! Zig `orelse return 1` arms.
//!
//! Surfaces the ported pipeline cannot yet serve are LOUD, never
//! silently skipped (docs/divergences/cli-commands.md):
//!   * `sab disasm` — the disassembler lives host-side in sci
//!     (`sci_bridge.disasmSabAlloc` -> sci/src/sab.zig
//!     disasmModule); no SAB decoder exists in these crates, so
//!     the read-error path is mirrored and real input is refused
//!     loudly instead of being faked.
//!
//! `skills` / `stability` ARE served: they run the A0-proven
//! metadata implementations in `skills_cmd` / `stability`.

use crate::cli_help::{write_command_help, HELP_TEXT};
use crate::delegate::{delegated_argv, run_child};
use crate::freshness::CompileOptions;
use crate::init_cmd::run_init_command;
use crate::options::{
    compiled_sa_test_passthrough, is_help_arg, parse_cli_options_from,
    parse_test_backend_from_args, sa_test_filter_from_args, sab_workspace_passthrough,
    CliOptions, TestBackend,
};
use crate::out::{lossy, zig_error_name};
use crate::pipeline::{compile_to_sa_string, compile_to_sab, run_frontend, test_filter_selects_no_tests, FrontendKind};
use crate::sab_paths::{
    default_output_path, has_emit_sab_arg, managed_sab_test_path, parse_out_file_arg,
    parse_sab_out_file_arg, write_managed_sab, write_sab_file,
};
use crate::workspace::resolve_sla_input_file;
use std::io::Write;

// -- helpers -------------------------------------------------------------------

/// Parse a command's options, handling the two early stops the
/// Zig original has: help requested (print help, code 0) and
/// option error (print help, code 1). Err carries the exit code.
fn options_or_stop(
    args: &[Vec<u8>],
    command: &str,
    start_idx: usize,
    stderr: &mut impl Write,
) -> Result<CliOptions, u8> {
    match parse_cli_options_from(args, command, start_idx) {
        Ok(options) => {
            if options.help_requested {
                let _ = write_command_help(stderr, command);
                Err(0)
            } else {
                Ok(options)
            }
        }
        Err(()) => {
            let _ = write_command_help(stderr, command);
            Err(1)
        }
    }
}

fn resolve_input(
    options: &CliOptions,
    stderr: &mut impl Write,
) -> Result<Option<String>, ()> {
    let source = options.source_file.as_ref().map(|f| lossy(f));
    let package = options.package_name.as_ref().map(|p| lossy(p));
    resolve_sla_input_file(stderr, source.as_deref(), package.as_deref())
}

fn extra_args(args: &[Vec<u8>], options: &CliOptions) -> Vec<Vec<u8>> {
    args[options.passthrough_start.min(args.len())..].to_vec()
}

fn extra_strings(args: &[Vec<u8>], options: &CliOptions) -> Vec<String> {
    extra_args(args, options).iter().map(|a| lossy(a)).collect()
}

/// ZigSys: write a sibling .sab from already-compiled bytes
/// (`writeSiblingSabFromBytes`).
fn write_sibling_sab(
    file: &str,
    sab_bytes: &[u8],
    stderr: &mut impl Write,
) -> Result<bool, ()> {
    let sab_out = default_output_path(file, ".sla", ".sab");
    write_sab_file(&sab_out, sab_bytes, stderr)
}


/// writeManagedSab with the binary's failure mapping: any
/// failure (reported or stream) stops the command with code 1.
fn managed_path(file: &str, sab_bytes: &[u8], stderr: &mut impl Write) -> Option<String> {
    write_managed_sab(file, sab_bytes, stderr).ok().flatten()
}

// -- build (text SA) --------------------------------------------------------------

pub fn run_build(args: &[Vec<u8>], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let options = match options_or_stop(args, "build", 1, stderr) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let file = match resolve_input(&options, stderr) {
        Ok(Some(f)) => f,
        Ok(None) => return 1,
        Err(()) => return 1,
    };
    // Zig build scans args from passthrough_start for --out/-o
    // (last one wins).
    let mut out_file: Option<String> = None;
    let mut idx = options.passthrough_start;
    while idx < args.len() {
        if args[idx] == b"--out" || args[idx] == b"-o" {
            if idx + 1 < args.len() {
                out_file = Some(lossy(&args[idx + 1]));
                idx += 1;
            }
        }
        idx += 1;
    }
    let final_out = out_file.unwrap_or_else(|| {
        if let Some(base) = file.strip_suffix(".sla") {
            format!("{base}.sa")
        } else {
            format!("{file}.sa")
        }
    });
    let Some(sa_code) = compile_to_sa_string(
        &file,
        FrontendKind::Compile,
        &CompileOptions::defaults(),
        &final_out,
        stderr,
    ) else {
        return 1;
    };
    if let Err(err) = std::fs::write(&final_out, sa_code.as_bytes()) {
        let _ = writeln!(
            stderr,
            "File Error: failed to write output {final_out}: error.{}",
            zig_error_name(&err)
        );
        return 1;
    }
    let _ = writeln!(
        stdout,
        "Sla Compiler: Successfully compiled {file} to {final_out}."
    );
    0
}

// -- check ---------------------------------------------------------------------------

pub fn run_check(args: &[Vec<u8>], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let options = match options_or_stop(args, "check", 1, stderr) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let file = match resolve_input(&options, stderr) {
        Ok(Some(f)) => f,
        Ok(None) => return 1,
        Err(()) => return 1,
    };
    if run_frontend(&file, FrontendKind::Check, None, stderr).is_none() {
        return 1;
    }
    let _ = writeln!(
        stdout,
        "Sla Compiler: Successfully parsed and verified syntax and types of {file}."
    );
    0
}

// -- delegated SAB builds (build-exe / build-wasm / build-js / build-c) ----------------

fn run_delegated_build(
    cmd: &str,
    args: &[Vec<u8>],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let _ = stdout;
    let options = match options_or_stop(args, cmd, 1, stderr) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let file = match resolve_input(&options, stderr) {
        Ok(Some(f)) => f,
        Ok(None) => return 1,
        Err(()) => return 1,
    };
    let extra = extra_strings(args, &options);
    let compile_options = if cmd == "build-wasm" {
        CompileOptions::build_wasm()
    } else {
        CompileOptions::build_exe()
    };
    let Some(sab_bytes) = compile_to_sab(&file, FrontendKind::Compile, &compile_options, stderr)
    else {
        return 1;
    };
    let Some(sab_out) = managed_path(&file, &sab_bytes, stderr) else {
        return 1;
    };
    if options.emit_sab_file {
        match write_sibling_sab(&file, &sab_bytes, stderr) {
            Ok(true) => {}
            _ => return 1,
        }
    }
    let argv = delegated_argv(cmd, &sab_out, &extra);
    run_child(&argv, &format!("'sa {cmd}'"), stderr)
}

pub fn run_build_workspace(
    args: &[Vec<u8>],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let _ = stdout;
    let options = match options_or_stop(args, "build-workspace", 1, stderr) {
        Ok(o) => o,
        Err(code) => return code,
    };
    if options.source_file.is_some() {
        let _ = write!(
            stderr,
            "Error: sla build-workspace does not accept a source file argument; run it from a workspace root or member directory\n"
        );
        return 1;
    }
    let package = options.package_name.as_ref().map(|p| lossy(p));
    let file = match crate::workspace::resolve_workspace_source_path(stderr, package.as_deref()) {
        Ok(Some(f)) => f,
        Ok(None) => return 1,
        Err(()) => return 1,
    };
    let extra = extra_strings(args, &options);
    let Some(sab_bytes) = compile_to_sab(
        &file,
        FrontendKind::Compile,
        &CompileOptions::build_exe(),
        stderr,
    ) else {
        return 1;
    };
    let Some(sab_out) = managed_path(&file, &sab_bytes, stderr) else {
        return 1;
    };
    if options.emit_sab_file {
        match write_sibling_sab(&file, &sab_bytes, stderr) {
            Ok(true) => {}
            _ => return 1,
        }
    }
    let argv = delegated_argv("build-exe", &sab_out, &extra);
    run_child(&argv, "'sa build-exe'", stderr)
}

// -- test --------------------------------------------------------------------------------

fn keep_generated_sa_test_input() -> bool {
    match std::env::var("SLA_KEEP_TEST_SA") {
        Ok(v) => !v.is_empty() && v != "0" && v != "false" && v != "no",
        Err(_) => false,
    }
}

pub fn run_test(args: &[Vec<u8>], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let options = match options_or_stop(args, "test", 1, stderr) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let file = match resolve_input(&options, stderr) {
        Ok(Some(f)) => f,
        Ok(None) => return 1,
        Err(()) => return 1,
    };
    let extra = extra_args(args, &options);
    let backend = match parse_test_backend_from_args(&extra, options.test_backend) {
        Ok(b) => b,
        Err(()) => {
            let _ = write_command_help(stderr, "test");
            return 1;
        }
    };
    let test_filter = sa_test_filter_from_args(&extra);
    if test_filter_selects_no_tests(&file, test_filter.as_deref(), stderr) == Some(true) {
        let _ = write!(stdout, "----\ntest result: ok. 0 passed; 0 failed; 0 skipped\n");
        return 0;
    }
    let compile_options = CompileOptions::test_codegen(test_filter.clone());
    let (test_path, delete_after) = match backend {
        TestBackend::Auto | TestBackend::Sab => {
            let sab_out = managed_sab_test_path(&file, &extra);
            let Some(sab_bytes) =
                compile_to_sab(&file, FrontendKind::TestCodegen, &compile_options, stderr)
            else {
                return 1;
            };
            match write_sab_file(&sab_out, &sab_bytes, stderr) {
                Ok(true) => {}
                _ => return 1,
            }
            if options.emit_sab_file
                && maybe_write_sibling_sab(&file, stderr).is_none()
            {
                return 1;
            }
            (sab_out, false)
        }
        TestBackend::Sa => {
            let sa_out = if let Some(base) = file.strip_suffix(".sla") {
                format!("{base}.test.sa")
            } else {
                format!("{file}.test.sa")
            };
            let Some(sa_code) = compile_to_sa_string(
                &file,
                FrontendKind::TestCodegen,
                &compile_options,
                &sa_out,
                stderr,
            ) else {
                return 1;
            };
            if let Err(err) = std::fs::write(&sa_out, sa_code.as_bytes()) {
                let _ = writeln!(
                    stderr,
                    "File Error: failed to write {sa_out}: error.{}",
                    zig_error_name(&err)
                );
                return 1;
            }
            if options.emit_sab_file
                && maybe_write_sibling_sab(&file, stderr).is_none()
            {
                return 1;
            }
            (sa_out, !keep_generated_sa_test_input())
        }
    };
    let passthrough = match compiled_sa_test_passthrough(&extra) {
        Ok(p) => p,
        Err(()) => {
            let _ = write_command_help(stderr, "test");
            if delete_after {
                let _ = std::fs::remove_file(&test_path);
            }
            return 1;
        }
    };
    let argv = delegated_argv("test", &test_path, &passthrough);
    let code = run_child(&argv, "'sa test'", stderr);
    if delete_after {
        let _ = std::fs::remove_file(&test_path);
    }
    code
}

/// Zig `maybeWriteSiblingSab` + its call-site error line in
/// compileSlaSabTestInput / compileSlaSaTestInput.
fn maybe_write_sibling_sab(file: &str, stderr: &mut impl Write) -> Option<()> {
    let sab_out = default_output_path(file, ".sla", ".sab");
    let bytes = compile_to_sab(
        file,
        FrontendKind::Compile,
        &CompileOptions::defaults(),
        stderr,
    )?;
    match write_sab_file(&sab_out, &bytes, stderr) {
        Ok(true) => Some(()),
        _ => {
            let _ = writeln!(
                stderr,
                "File Error: failed to emit sibling SAB for {file}: error.InvalidFormat"
            );
            None
        }
    }
}

// -- sab dispatcher -------------------------------------------------------------------------

pub fn run_sab_command(
    args: &[Vec<u8>],
    subcommand_index: usize,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    if subcommand_index >= args.len() || is_help_arg(&args[subcommand_index]) {
        let _ = write_command_help(stderr, "sab");
        return if subcommand_index < args.len() { 0 } else { 1 };
    }
    let subcmd = &args[subcommand_index];
    let option_start = subcommand_index + 1;
    if subcmd == b"build" {
        return run_sab_build(args, option_start, stdout, stderr);
    }
    if subcmd == b"workspace" {
        return run_sab_workspace(args, option_start, stdout, stderr);
    }
    if subcmd == b"disasm" {
        return run_sab_disasm(args, option_start, stdout, stderr);
    }
    let _ = writeln!(stderr, "Unknown sla sab command: {}", lossy(subcmd));
    let _ = write_command_help(stderr, "sab");
    1
}

fn run_sab_build(
    args: &[Vec<u8>],
    option_start: usize,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let options = match options_or_stop(args, "sab build", option_start, stderr) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let file = match resolve_input(&options, stderr) {
        Ok(Some(f)) => f,
        Ok(None) => return 1,
        Err(()) => return 1,
    };
    let Some(sab_bytes) = compile_to_sab(
        &file,
        FrontendKind::Compile,
        &CompileOptions::defaults(),
        stderr,
    ) else {
        return 1;
    };
    let Some(managed_path) = managed_path(&file, &sab_bytes, stderr) else {
        return 1;
    };
    if let Some(final_out) = parse_out_file_arg(args, option_start) {
        match write_sab_file(&final_out, &sab_bytes, stderr) {
            Ok(true) => {}
            _ => return 1,
        }
        let _ = writeln!(
            stdout,
            "Sla Compiler: Successfully compiled {file} to SAB {final_out} (managed cache {managed_path})."
        );
    } else {
        let _ = writeln!(
            stdout,
            "Sla Compiler: Successfully compiled {file} to managed SAB {managed_path}."
        );
    }
    0
}

fn run_sab_workspace(
    args: &[Vec<u8>],
    option_start: usize,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let _ = stdout;
    let options = match options_or_stop(args, "sab workspace", option_start, stderr) {
        Ok(o) => o,
        Err(code) => return code,
    };
    if options.source_file.is_some() {
        let _ = write!(
            stderr,
            "Error: sla sab workspace does not accept a source file argument; run it from a workspace root or member directory\n"
        );
        return 1;
    }
    let package = options.package_name.as_ref().map(|p| lossy(p));
    let file = match crate::workspace::resolve_workspace_source_path(stderr, package.as_deref()) {
        Ok(Some(f)) => f,
        Ok(None) => return 1,
        Err(()) => return 1,
    };
    let extra = extra_args(args, &options);
    let Some(sab_bytes) = compile_to_sab(
        &file,
        FrontendKind::Compile,
        &CompileOptions::build_exe(),
        stderr,
    ) else {
        return 1;
    };
    let Some(managed_path) = managed_path(&file, &sab_bytes, stderr) else {
        return 1;
    };
    if let Some(sab_out) = parse_sab_out_file_arg(args, option_start) {
        match write_sab_file(&sab_out, &sab_bytes, stderr) {
            Ok(true) => {}
            _ => return 1,
        }
    }
    if options.emit_sab_file || has_emit_sab_arg(args, option_start) {
        match write_sibling_sab(&file, &sab_bytes, stderr) {
            Ok(true) => {}
            _ => return 1,
        }
    }
    let passthrough = sab_workspace_passthrough(&extra);
    let argv = delegated_argv("build-exe", &managed_path, &passthrough);
    run_child(&argv, "'sa build-exe' for SAB workspace output", stderr)
}

fn run_sab_disasm(
    args: &[Vec<u8>],
    option_start: usize,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let _ = stdout;
    if option_start >= args.len() || is_help_arg(&args[option_start]) {
        let _ = write_command_help(stderr, "sab disasm");
        return if option_start < args.len() { 0 } else { 1 };
    }
    let file = lossy(&args[option_start]);
    // Zig reads the file first, with this exact error line; the
    // disassembly itself is host-side (sci_bridge) and unported.
    match std::fs::read(&file) {
        Ok(bytes) => {
            if bytes.len() > 16 * 1024 * 1024 {
                let _ = writeln!(
                    stderr,
                    "SAB Error: failed to read {file}: error.StreamTooLong"
                );
                return 1;
            }
            let _ = writeln!(
                stderr,
                "SAB Error: sab disasm is not available in the standalone sla binary yet (the disassembler lives in the sa host: sci/src/sab.zig disasmModule)"
            );
            1
        }
        Err(err) => {
            let _ = writeln!(
                stderr,
                "SAB Error: failed to read {file}: error.{}",
                zig_error_name(&err)
            );
            1
        }
    }
}

// -- top-level dispatch -------------------------------------------------------------------------

/// Mirror of `runSlaCommandImpl`'s dispatch for the standalone
/// binary: `args` starts at the command word (the binary name is
/// already stripped). Unknown commands print
/// `Unknown sla command: {cmd}` and return 1; a missing command
/// prints the one-line usage and returns 1.
/// skills / stability: the A0-proven metadata implementations,
/// driven with a synthetic `sa sla` argv prefix so their
/// option_start conventions carry over unchanged. A stream
/// failure is an invocation failure (exit 1) — the standalone
/// form of the descriptor path's Failed status.
fn run_metadata_command(
    skills: bool,
    args: &[Vec<u8>],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let mut full: Vec<Vec<u8>> = vec![b"sa".to_vec(), b"sla".to_vec()];
    full.extend(args.iter().cloned());
    let mut out_sink = crate::cmdio::IoSink(stdout as &mut dyn Write);
    let mut err_sink = crate::cmdio::IoSink(stderr as &mut dyn Write);
    let result = if skills {
        crate::skills_cmd::run_skills_command(&full, 3, &mut out_sink, &mut err_sink, false)
    } else {
        crate::stability::run_stability_command(&full, 3, &mut out_sink, &mut err_sink, false)
    };
    result.unwrap_or(1)
}

pub fn dispatch(args: &[Vec<u8>], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    if args.is_empty() {
        let _ = write!(stderr, "usage: sa sla <command> [options]\n");
        return 1;
    }
    if args[0] == b"slab" {
        return run_sab_command(args, 1, stdout, stderr);
    }
    let cmd = &args[0];
    if cmd == b"help" {
        let _ = write!(stderr, "{HELP_TEXT}");
        return 0;
    }
    if cmd == b"init" {
        return run_init_command(args, 1, stdout, stderr);
    }
    if cmd == b"skills" {
        return run_metadata_command(true, args, stdout, stderr);
    }
    if cmd == b"stability" {
        return run_metadata_command(false, args, stdout, stderr);
    }
    if cmd == b"sab" {
        return run_sab_command(args, 1, stdout, stderr);
    }
    if cmd == b"build" {
        return run_build(args, stdout, stderr);
    }
    if cmd == b"check" {
        return run_check(args, stdout, stderr);
    }
    if cmd == b"test" {
        return run_test(args, stdout, stderr);
    }
    if cmd == b"build-workspace" {
        return run_build_workspace(args, stdout, stderr);
    }
    for delegated in ["build-exe", "build-wasm", "build-js", "build-c"] {
        if cmd == delegated.as_bytes() {
            return run_delegated_build(delegated, args, stdout, stderr);
        }
    }
    let _ = writeln!(stderr, "Unknown sla command: {}", lossy(cmd));
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> (u8, String, String) {
        let argv: Vec<Vec<u8>> = args.iter().map(|s| s.as_bytes().to_vec()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = dispatch(&argv, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn dispatch_basics() {
        let (code, _, err) = run(&[]);
        assert_eq!(code, 1);
        assert_eq!(err, "usage: sa sla <command> [options]\n");
        let (code, _, err) = run(&["help"]);
        assert_eq!(code, 0);
        assert!(err.starts_with("usage: sla <command> [options]\n"));
        let (code, _, err) = run(&["nosuchcmd"]);
        assert_eq!(code, 1);
        assert_eq!(err, "Unknown sla command: nosuchcmd\n");
        let (code, _, err) = run(&["sab"]);
        assert_eq!(code, 1);
        assert!(err.starts_with("usage: sa sla sab <build|workspace|disasm>"));
        let (code, _, err) = run(&["check", "--help"]);
        assert_eq!(code, 0);
        assert!(err.starts_with("usage: sa sla check [file]"));
        let (code, _, err) = run(&["check", "-p"]);
        assert_eq!(code, 1);
        assert!(err.starts_with("usage: sa sla check [file]"));
        let (code, _, err) = run(&["slab", "zzz"]);
        assert_eq!(code, 1);
        assert!(err.starts_with("Unknown sla sab command: zzz\n"));
    }

    #[test]
    fn metadata_commands_run() {
        let (code, out, _) = run(&["skills", "--json"]);
        assert_eq!(code, 0);
        assert!(out.starts_with("{\"status\":\"ok\",\"skills\":["));
        let (code, out, _) = run(&["stability", "schema"]);
        assert_eq!(code, 0);
        assert!(out.contains("stability-metadata.schema.json"));
        let (code, _, err) = run(&["stability"]);
        assert_eq!(code, 1);
        assert!(err.starts_with("usage: sa sla stability"));
    }

    #[test]
    fn disasm_missing_file_mirrors_zig_read_error() {
        let (code, _, err) = run(&["sab", "disasm", "/nonexistent/x.sab"]);
        assert_eq!(code, 1);
        assert!(
            err.starts_with("SAB Error: failed to read /nonexistent/x.sab: error.FileNotFound"),
            "{err}"
        );
    }
}
