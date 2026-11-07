//! CLI option parsing — mirror of `plugin_cli.zig`'s
//! `parseSlaCliOptionsFrom` / `parseTestBackendFromArgs` /
//! `saTestFilterFromArgs` and the two passthrough builders
//! (venue head 2cc12ea8). Conventions: argv here INCLUDES the
//! command word at index 0 (i.e. args after the binary name start
//! with the subcommand), so `start_idx` for a top-level command is
//! 1 and for `sab <sub>` is 2. Err(()) is `error.InvalidFormat`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestBackend {
    Auto,
    Sab,
    Sa,
}

#[derive(Debug)]
pub struct CliOptions {
    pub package_name: Option<Vec<u8>>,
    pub source_file: Option<Vec<u8>>,
    pub passthrough_start: usize,
    pub help_requested: bool,
    pub emit_sab_file: bool,
    pub test_backend: TestBackend,
}

pub fn is_help_arg(arg: &[u8]) -> bool {
    arg == b"-h" || arg == b"--help"
}

fn parse_test_backend_value(value: &[u8]) -> Result<TestBackend, ()> {
    match value {
        b"auto" => Ok(TestBackend::Auto),
        b"sab" => Ok(TestBackend::Sab),
        b"sa" => Ok(TestBackend::Sa),
        _ => Err(()),
    }
}

/// Zig `parseSlaCliOptionsFrom`.
pub fn parse_cli_options_from(
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

/// Zig `parseTestBackendFromArgs`: re-scans the passthrough args
/// (after the source file) for another --test-backend, which wins
/// over the pre-file one carried in CliOptions.
pub fn parse_test_backend_from_args(
    args: &[Vec<u8>],
    default_backend: TestBackend,
) -> Result<TestBackend, ()> {
    let mut backend = default_backend;
    let mut idx = 0;
    while idx < args.len() {
        let arg = &args[idx];
        if arg == b"--test-backend" {
            idx += 1;
            if idx >= args.len() {
                return Err(());
            }
            backend = parse_test_backend_value(&args[idx])?;
            idx += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix(b"--test-backend=".as_slice()) {
            backend = parse_test_backend_value(value)?;
        }
        idx += 1;
    }
    Ok(backend)
}

/// Zig `saTestFilterFromArgs`.
pub fn sa_test_filter_from_args(args: &[Vec<u8>]) -> Option<String> {
    let mut idx = 0;
    while idx < args.len() {
        let arg = &args[idx];
        if arg == b"--filter" {
            if idx + 1 < args.len() && !args[idx + 1].is_empty() {
                return Some(crate::out::lossy(&args[idx + 1]));
            }
            return None;
        }
        if let Some(value) = arg.strip_prefix(b"--filter=".as_slice()) {
            if !value.is_empty() {
                return Some(crate::out::lossy(value));
            }
            return None;
        }
        idx += 1;
    }
    None
}

/// Zig `appendCompiledSaTestPassthrough` (strip-filter mode):
/// drops `--test-backend <v>` / `--test-backend=<v>` and
/// `--filter <v>` / `--filter=<v>`; a trailing bare flag is
/// error.InvalidFormat. (`appendDefaultJobsAuto` is a deliberate
/// no-op in the original, so nothing is appended after.)
pub fn compiled_sa_test_passthrough(args: &[Vec<u8>]) -> Result<Vec<String>, ()> {
    let mut out: Vec<String> = Vec::new();
    let mut idx = 0;
    while idx < args.len() {
        let arg = &args[idx];
        if arg == b"--test-backend" || arg == b"--filter" {
            idx += 1;
            if idx >= args.len() {
                return Err(());
            }
            idx += 1;
            continue;
        }
        if arg.starts_with(b"--test-backend=") || arg.starts_with(b"--filter=") {
            idx += 1;
            continue;
        }
        out.push(crate::out::lossy(arg));
        idx += 1;
    }
    Ok(out)
}

/// Zig `appendSabWorkspacePassthrough`: drops `--emit-sab[-file]`
/// and `--sab-out <path>` pairs.
pub fn sab_workspace_passthrough(args: &[Vec<u8>]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut idx = 0;
    while idx < args.len() {
        let arg = &args[idx];
        if arg == b"--emit-sab" || arg == b"--emit-sab-file" {
            idx += 1;
            continue;
        }
        if arg == b"--sab-out" {
            idx += 2; // flag + value (overshoot at the tail is inert)
            continue;
        }
        out.push(crate::out::lossy(arg));
        idx += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(args: &[&str]) -> Vec<Vec<u8>> {
        args.iter().map(|s| s.as_bytes().to_vec()).collect()
    }

    #[test]
    fn compiled_test_passthrough_strips_selection_args() {
        // The plugin_cli.zig unit test, ported.
        let out = compiled_sa_test_passthrough(&v(&[
            "--filter",
            "one",
            "--trace-panic",
            "--jobs=1",
            "--test-backend",
            "sab",
        ]))
        .unwrap();
        assert_eq!(out, vec!["--trace-panic", "--jobs=1"]);
        let out = compiled_sa_test_passthrough(&v(&["--filter=one", "--trace-panic"])).unwrap();
        assert_eq!(out, vec!["--trace-panic"]);
        assert!(compiled_sa_test_passthrough(&v(&["--filter"])).is_err());
    }

    #[test]
    fn sab_workspace_passthrough_strips_sab_outs() {
        // The plugin_sab_paths.zig unit test, ported.
        let out = sab_workspace_passthrough(&v(&["--sab-out", "/tmp/out.sab", "-o", "/tmp/app"]));
        assert_eq!(out, vec!["-o", "/tmp/app"]);
    }

    #[test]
    fn options_parse_shapes() {
        let args = v(&["build", "-p", "pkg", "src.sla", "--out", "x"]);
        let o = parse_cli_options_from(&args, "build", 1).unwrap();
        assert_eq!(o.package_name.as_deref(), Some(b"pkg".as_slice()));
        assert_eq!(o.source_file.as_deref(), Some(b"src.sla".as_slice()));
        assert_eq!(o.passthrough_start, 4);
        // Help after the source file is NOT seen (parse breaks).
        let args = v(&["build", "src.sla", "-h"]);
        let o = parse_cli_options_from(&args, "build", 1).unwrap();
        assert!(!o.help_requested);
        // Missing -p value.
        let args = v(&["check", "-p"]);
        assert!(parse_cli_options_from(&args, "check", 1).is_err());
    }

    #[test]
    fn filter_and_backend_scans() {
        assert_eq!(
            sa_test_filter_from_args(&v(&["--filter", "keep"])).as_deref(),
            Some("keep")
        );
        assert_eq!(
            sa_test_filter_from_args(&v(&["--filter=keep"])).as_deref(),
            Some("keep")
        );
        assert_eq!(sa_test_filter_from_args(&v(&["--filter="])), None);
        assert_eq!(
            parse_test_backend_from_args(&v(&["--test-backend", "sa"]), TestBackend::Auto)
                .unwrap(),
            TestBackend::Sa
        );
        assert!(parse_test_backend_from_args(&v(&["--test-backend"]), TestBackend::Auto).is_err());
    }
}
