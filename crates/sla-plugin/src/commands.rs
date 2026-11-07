//! Command dispatch — `runSlaCommandImpl` (plugin_commands.zig,
//! venue head 6002b386) for the A0 slice: the commands that do not
//! drive the compile pipeline (`help`, `init`, `skills`,
//! `stability`, the `sab` dispatcher's help/error paths, and every
//! command's `-h/--help` / option-error path), plus the unknown
//! command paths.
//!
//! Result mapping mirrors `runSlaCommandAbi` (plugin.zig):
//! `Code(c)` -> AbiStatus.ok with out_code = c; `Unknown` ->
//! AbiStatus.unknown_command (out_code stays 0); `Fail` ->
//! AbiStatus.failed (an invocation-level failure: a stream write
//! error, or a pipeline command whose pipeline is not wired in
//! this slice — pipeline commands keep the previous stub's outward
//! shape, Failed with no output, until their pipelines land).

use crate::cli_help::write_command_help;
use crate::cmdio::CmdWrite;
use crate::init_cmd::run_init_command;
use crate::skills_cmd::run_skills_command;
use crate::stability::run_stability_command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchResult {
    Code(u8),
    Unknown,
    Fail,
}

fn is_help_arg(arg: &[u8]) -> bool {
    arg == b"-h" || arg == b"--help"
}

fn lossy(arg: &[u8]) -> String {
    String::from_utf8_lossy(arg).into_owned()
}

// -- parseSlaCliOptionsFrom (plugin_cli.zig) -------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestBackend {
    Auto,
    Sab,
    Sa,
}

#[derive(Debug)]
struct CliOptions {
    package_name: Option<Vec<u8>>,
    source_file: Option<Vec<u8>>,
    passthrough_start: usize,
    help_requested: bool,
    emit_sab_file: bool,
    test_backend: TestBackend,
}

fn parse_test_backend_value(value: &[u8]) -> Result<TestBackend, ()> {
    match value {
        b"auto" => Ok(TestBackend::Auto),
        b"sab" => Ok(TestBackend::Sab),
        b"sa" => Ok(TestBackend::Sa),
        _ => Err(()),
    }
}

/// Zig `parseSlaCliOptionsFrom`: Err(()) is `error.InvalidFormat`.
fn parse_cli_options_from(
    args: &[Vec<u8>],
    command: &str,
    start_idx: usize,
) -> Result<CliOptions, ()> {
    let mut options = CliOptions {
        package_name: None,
        source_file: None,
        passthrough_start: args.len(),
        help_requested: false,
        emit_sab_file: false,
        test_backend: TestBackend::Auto,
    };
    let mut idx = start_idx;
    while idx < args.len() {
        let arg = &args[idx];
        if is_help_arg(arg) {
            options.help_requested = true;
            idx += 1;
            continue;
        }
        if arg == b"--emit-sab" || arg == b"--emit-sab-file" {
            options.emit_sab_file = true;
            idx += 1;
            continue;
        }
        if command == "test" {
            if let Some(value) = arg.strip_prefix(b"--test-backend=".as_slice()) {
                options.test_backend = parse_test_backend_value(value)?;
                idx += 1;
                continue;
            }
            if arg == b"--test-backend" {
                idx += 1;
                if idx >= args.len() {
                    return Err(());
                }
                options.test_backend = parse_test_backend_value(&args[idx])?;
                idx += 1;
                continue;
            }
        }
        if let Some(value) = arg.strip_prefix(b"--package=".as_slice()) {
            options.package_name = Some(value.to_vec());
            idx += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix(b"-p=".as_slice()) {
            options.package_name = Some(value.to_vec());
            idx += 1;
            continue;
        }
        if arg == b"--package" || arg == b"-p" {
            idx += 1;
            if idx >= args.len() {
                return Err(());
            }
            options.package_name = Some(args[idx].clone());
            idx += 1;
            continue;
        }
        if options.source_file.is_none() && !arg.starts_with(b"-") {
            options.source_file = Some(arg.clone());
            options.passthrough_start = idx + 1;
            break;
        }
        options.passthrough_start = idx;
        break;
    }
    Ok(options)
}

// -- the top-level help text (plugin_commands.zig `help` branch) -----------

const HELP_TEXT: &str = "usage: sla <command> [options]\n       sa sla <command> [options]\n\nCommands:\n  init       [path]\n  skills     [--json]\n  stability  schema|verify ...\n  build      [file] [-p <package>] [--out <file>]\n  build-workspace [-p <package>] [sa-build-exe args]\n  build-exe  [file] [-p <package>] [sa-build-exe args]\n  build-wasm [file] [-p <package>] [sa-build-wasm args]\n  build-js   [file] [-p <package>] [sa-build-js args]\n  build-c    [file] [-p <package>] [sa-build-c args]\n  sab build  [file] [-p <package>] [--out <file.sab>]\n  sab workspace [-p <package>] [--sab-out <file.sab>] [sa-build-exe args]\n  sab disasm <file.sab> [--out <file.sa>]\n  slab build|workspace|disasm ...    Short alias\n  check      [file] [-p <package>]\n  test       [file] [-p <package>] [--test-backend auto|sab|sa] [sa-test args]\n";

// -- sab dispatcher (help / error paths only in this slice) -----------------

fn run_sab_command(
    args: &[Vec<u8>],
    subcommand_index: usize,
    // The sab dispatcher's help/error paths write to stderr only.
    _stdout: &mut impl CmdWrite,
    stderr: &mut impl CmdWrite,
) -> Result<DispatchResult, ()> {
    if subcommand_index >= args.len() || is_help_arg(&args[subcommand_index]) {
        write_command_help(stderr, "sab")?;
        return Ok(DispatchResult::Code(if subcommand_index < args.len() {
            0
        } else {
            1
        }));
    }
    let subcmd = &args[subcommand_index];
    let option_start = subcommand_index + 1;
    let sub_name: &[u8] = subcmd;
    if sub_name == b"build" || sub_name == b"workspace" {
        let command = if sub_name == b"build" {
            "sab build"
        } else {
            "sab workspace"
        };
        match parse_cli_options_from(args, command, option_start) {
            Err(()) => {
                write_command_help(stderr, command)?;
                return Ok(DispatchResult::Code(1));
            }
            Ok(options) => {
                if options.help_requested {
                    write_command_help(stderr, command)?;
                    return Ok(DispatchResult::Code(0));
                }
            }
        }
        return Ok(DispatchResult::Fail); // SAB pipeline not wired
    }
    if sub_name == b"disasm" {
        if option_start >= args.len() || is_help_arg(&args[option_start]) {
            write_command_help(stderr, "sab disasm")?;
            return Ok(DispatchResult::Code(if option_start < args.len() {
                0
            } else {
                1
            }));
        }
        return Ok(DispatchResult::Fail); // host disasm bridge not wired
    }
    stderr.print(format_args!(
        "Unknown sla sab command: {}\n",
        lossy(subcmd)
    ))?;
    write_command_help(stderr, "sab")?;
    Ok(DispatchResult::Code(1))
}

// -- pipeline commands: options + help paths, then loud Fail ----------------

fn run_pipeline_command(
    cmd: &str,
    args: &[Vec<u8>],
    stderr: &mut impl CmdWrite,
) -> Result<DispatchResult, ()> {
    let options = match parse_cli_options_from(args, cmd, 3) {
        Ok(o) => o,
        Err(()) => {
            write_command_help(stderr, cmd)?;
            return Ok(DispatchResult::Code(1));
        }
    };
    if options.help_requested {
        write_command_help(stderr, cmd)?;
        return Ok(DispatchResult::Code(0));
    }
    if cmd == "build-workspace" && options.source_file.is_some() {
        stderr.write_str("Error: sla build-workspace does not accept a source file argument; run it from a workspace root or member directory\n")?;
        return Ok(DispatchResult::Code(1));
    }
    // The compile pipeline is not wired into this cdylib yet.
    let _ = options;
    Ok(DispatchResult::Fail)
}

// -- top-level dispatch ------------------------------------------------------

/// Zig `runSlaCommandImpl`. Err(()) = invocation failure (a stream
/// write failed); the caller maps the three outcomes onto
/// (AbiStatus, out_code) exactly as `runSlaCommandAbi` does.
pub fn dispatch(
    json_mode: bool,
    args: &[Vec<u8>],
    stdout: &mut impl CmdWrite,
    stderr: &mut impl CmdWrite,
) -> Result<DispatchResult, ()> {
    if args.len() < 2 {
        return Ok(DispatchResult::Unknown);
    }
    if args[1] == b"slab" {
        return run_sab_command(args, 2, stdout, stderr);
    }
    if args[1] != b"sla" {
        return Ok(DispatchResult::Unknown);
    }
    if args.len() < 3 {
        stderr.write_str("usage: sa sla <command> [options]\n")?;
        return Ok(DispatchResult::Code(1));
    }
    let cmd = &args[2];
    if cmd == b"help" {
        stderr.write_str(HELP_TEXT)?;
        return Ok(DispatchResult::Code(0));
    }
    if cmd == b"init" {
        return run_init_command(args, 3, stdout, stderr).map(DispatchResult::Code);
    }
    if cmd == b"skills" {
        return run_skills_command(args, 3, stdout, stderr, json_mode).map(DispatchResult::Code);
    }
    if cmd == b"stability" {
        return run_stability_command(args, 3, stdout, stderr, json_mode)
            .map(DispatchResult::Code);
    }
    if cmd == b"sab" {
        return run_sab_command(args, 3, stdout, stderr);
    }
    for pipeline in [
        "build",
        "build-exe",
        "build-wasm",
        "build-js",
        "build-c",
        "build-workspace",
        "check",
        "test",
    ] {
        if cmd == pipeline.as_bytes() {
            return run_pipeline_command(pipeline, args, stderr);
        }
    }
    stderr.print(format_args!("Unknown sla command: {}\n", lossy(cmd)))?;
    Ok(DispatchResult::Code(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> (DispatchResult, String, String) {
        let argv: Vec<Vec<u8>> = args.iter().map(|s| s.as_bytes().to_vec()).collect();
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let result = dispatch(false, &argv, &mut out, &mut err).unwrap();
        (
            result,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn no_command_and_foreign_command_are_unknown() {
        assert_eq!(run(&["sa"]).0, DispatchResult::Unknown);
        assert_eq!(run(&["sa", "other", "x"]).0, DispatchResult::Unknown);
        let (result, _, err) = run(&["sa", "sla"]);
        assert_eq!(result, DispatchResult::Code(1));
        assert_eq!(err, "usage: sa sla <command> [options]\n");
    }

    #[test]
    fn help_goes_to_stderr_with_code_0() {
        let (result, out, err) = run(&["sa", "sla", "help"]);
        assert_eq!(result, DispatchResult::Code(0));
        assert!(out.is_empty());
        assert!(err.starts_with("usage: sla <command> [options]\n"));
        assert!(err.contains("  test       [file]"));
    }

    #[test]
    fn unknown_command_is_code_1() {
        let (result, _, err) = run(&["sa", "sla", "nosuchcmd"]);
        assert_eq!(result, DispatchResult::Code(1));
        assert_eq!(err, "Unknown sla command: nosuchcmd\n");
    }

    #[test]
    fn pipeline_help_and_option_error_paths() {
        let (result, _, err) = run(&["sa", "sla", "build", "--help"]);
        assert_eq!(result, DispatchResult::Code(0));
        assert!(err.starts_with("usage: sa sla build [file]"));
        // A file before -h hides the help request (parse breaks at
        // the source file), so this reaches the unwired pipeline.
        let (result, _, _) = run(&["sa", "sla", "build", "x.sla", "-h"]);
        assert_eq!(result, DispatchResult::Fail);
        // Missing -p value is an option error: help + code 1.
        let (result, _, err) = run(&["sa", "sla", "check", "-p"]);
        assert_eq!(result, DispatchResult::Code(1));
        assert!(err.starts_with("usage: sa sla check [file]"));
        // build-workspace rejects a source file before the pipeline.
        let (result, _, err) = run(&["sa", "sla", "build-workspace", "x.sla"]);
        assert_eq!(result, DispatchResult::Code(1));
        assert!(err.contains("does not accept a source file argument"));
    }

    #[test]
    fn sab_dispatch_paths() {
        let (result, _, err) = run(&["sa", "sla", "sab"]);
        assert_eq!(result, DispatchResult::Code(1));
        assert!(err.starts_with("usage: sa sla sab <build|workspace|disasm>"));
        let (result, _, err) = run(&["sa", "slab", "zzz"]);
        assert_eq!(result, DispatchResult::Code(1));
        assert!(err.starts_with("Unknown sla sab command: zzz\nusage: sa sla sab"));
        let (result, _, _) = run(&["sa", "sla", "sab", "build", "x.sla"]);
        assert_eq!(result, DispatchResult::Fail);
        let (result, _, err) = run(&["sa", "sla", "sab", "disasm"]);
        assert_eq!(result, DispatchResult::Code(1));
        assert!(err.starts_with("usage: sa sla sab disasm"));
    }
}
