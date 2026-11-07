//! The `stability` command, mirrored from
//! `runSlaStabilityCommand` (plugin_cli.zig) +
//! `stability_metadata.zig`'s schema constant and report writers.
//! Ported into sla-cli from sla-plugin's A0-proven mirror
//! (baseline d811673c) unchanged in behavior.
//!
//! The schema output is byte-pinned: `SCHEMA_JSON` below is the
//! Zig multiline literal extracted mechanically from
//! stability_metadata.zig, and `schema_json + '\n'` hashes to the
//! venue golden sha256
//! 99bf7d0459f3ee93ace446c571e7075265daf6fe27d1ba797e26f88f934a4d11.

use crate::cmdio::{write_command_help, zig_error_name, CmdWrite};
use crate::stability_validate::{validate_manifest_text, ValidationReport};

pub const SCHEMA_JSON: &str = r#"
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://sa.dev/sla/stability-metadata.schema.json",
  "title": "SLA downstream stability metadata",
  "type": "object",
  "required": ["schema_version", "labels", "artifacts"],
  "properties": {
    "schema_version": { "const": 1 },
    "project": { "type": "string", "minLength": 1 },
    "labels": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "description"],
        "properties": {
          "name": { "type": "string", "pattern": "^[A-Za-z0-9][A-Za-z0-9_.-]*$" },
          "description": { "type": "string", "minLength": 1 }
        },
        "additionalProperties": false
      },
      "uniqueItems": true
    },
    "artifacts": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["path", "labels"],
        "properties": {
          "path": { "type": "string", "minLength": 1 },
          "labels": { "type": "array", "items": { "type": "string" } },
          "evidence": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["kind", "status"],
              "properties": {
                "kind": { "type": "string", "minLength": 1 },
                "status": { "enum": ["pass", "fail", "unknown"] },
                "command": { "type": "string" },
                "note": { "type": "string" }
              },
              "additionalProperties": false
            }
          }
        },
        "additionalProperties": false
      }
    }
  },
  "additionalProperties": false
}
"#;

/// Zig `writeJsonString` (stability_metadata variant): only
/// `\\`, `\"`, `\n`, `\r`, `\t` are escaped; every other byte —
/// including control bytes — passes through raw.
fn write_json_string(w: &mut impl CmdWrite, value: &[u8]) -> Result<(), ()> {
    w.write_all(b"\"")?;
    for &c in value {
        match c {
            b'\\' => w.write_all(b"\\\\")?,
            b'"' => w.write_all(b"\\\"")?,
            b'\n' => w.write_all(b"\\n")?,
            b'\r' => w.write_all(b"\\r")?,
            b'\t' => w.write_all(b"\\t")?,
            _ => w.write_all(&[c])?,
        }
    }
    w.write_all(b"\"")
}

/// Zig `writeReportJson`.
pub fn write_report_json(w: &mut impl CmdWrite, report: &ValidationReport) -> Result<(), ()> {
    w.write_str("{\"status\":")?;
    write_json_string(w, if report.valid { b"ok" } else { b"error" })?;
    w.print(format_args!(
        ",\"valid\":{},\"labels\":{},\"artifacts\":{},\"evidence\":{},\"errors\":[",
        report.valid, report.label_count, report.artifact_count, report.evidence_count
    ))?;
    for (idx, message) in report.errors.iter().enumerate() {
        if idx != 0 {
            w.write_all(b",")?;
        }
        write_json_string(w, message.as_bytes())?;
    }
    w.write_str("]}\n")
}

/// Zig `writeReportText`.
pub fn write_report_text(w: &mut impl CmdWrite, report: &ValidationReport) -> Result<(), ()> {
    if report.valid {
        w.write_str("stability metadata manifest ok\n")?;
    } else {
        w.write_str("stability metadata manifest invalid\n")?;
    }
    w.print(format_args!(
        "labels: {}\nartifacts: {}\nevidence: {}\n",
        report.label_count, report.artifact_count, report.evidence_count
    ))?;
    for message in &report.errors {
        w.print(format_args!("error: {message}\n"))?;
    }
    Ok(())
}

fn is_help_arg(arg: &[u8]) -> bool {
    arg == b"-h" || arg == b"--help"
}

/// Zig `runSlaStabilityCommand`. Err(()) = invocation failure
/// (stream error), as with the Zig error union.
pub fn run_stability_command(
    args: &[Vec<u8>],
    option_start: usize,
    stdout: &mut impl CmdWrite,
    stderr: &mut impl CmdWrite,
    default_json_mode: bool,
) -> Result<u8, ()> {
    if option_start >= args.len() {
        write_command_help(stderr, "stability")?;
        return Ok(1);
    }
    let subcmd = &args[option_start];
    if is_help_arg(subcmd) {
        write_command_help(stderr, "stability")?;
        return Ok(0);
    }

    if subcmd == b"schema" {
        let mut idx = option_start + 1;
        while idx < args.len() {
            let arg = &args[idx];
            if is_help_arg(arg) {
                write_command_help(stderr, "stability schema")?;
                return Ok(0);
            }
            if arg == b"--json" {
                idx += 1;
                continue;
            }
            stderr.print(format_args!(
                "Unknown sla stability schema option: {}\n",
                String::from_utf8_lossy(arg)
            ))?;
            write_command_help(stderr, "stability schema")?;
            return Ok(1);
        }
        stdout.write_str(SCHEMA_JSON)?;
        stdout.write_all(b"\n")?;
        return Ok(0);
    }

    if subcmd == b"verify" {
        let mut json_mode = default_json_mode;
        let mut manifest_path: Option<&Vec<u8>> = None;
        let mut idx = option_start + 1;
        while idx < args.len() {
            let arg = &args[idx];
            if is_help_arg(arg) {
                write_command_help(stderr, "stability verify")?;
                return Ok(0);
            }
            if arg == b"--json" {
                json_mode = true;
                idx += 1;
                continue;
            }
            if arg.starts_with(b"-") {
                stderr.print(format_args!(
                    "Unknown sla stability verify option: {}\n",
                    String::from_utf8_lossy(arg)
                ))?;
                write_command_help(stderr, "stability verify")?;
                return Ok(1);
            }
            if manifest_path.is_some() {
                stderr.print(format_args!(
                    "Unexpected sla stability verify argument: {}\n",
                    String::from_utf8_lossy(arg)
                ))?;
                write_command_help(stderr, "stability verify")?;
                return Ok(1);
            }
            manifest_path = Some(arg);
            idx += 1;
        }
        let Some(path) = manifest_path else {
            stderr.write_str("Missing stability manifest path\n")?;
            write_command_help(stderr, "stability verify")?;
            return Ok(1);
        };
        let path_str = String::from_utf8_lossy(path).into_owned();
        let manifest = match read_manifest(&path_str) {
            Ok(bytes) => bytes,
            Err(name) => {
                // Zig prints the error with {} -> `error.<Name>`.
                stderr.print(format_args!(
                    "File Error: failed to read {path_str}: error.{name}\n"
                ))?;
                return Ok(1);
            }
        };
        let report = validate_manifest_text(&manifest);
        if json_mode {
            write_report_json(stdout, &report)?;
        } else {
            write_report_text(stdout, &report)?;
        }
        return Ok(if report.valid { 0 } else { 1 });
    }

    stderr.print(format_args!(
        "Unknown sla stability command: {}\n",
        String::from_utf8_lossy(subcmd)
    ))?;
    write_command_help(stderr, "stability")?;
    Ok(1)
}

/// Zig `readFileAlloc(path, 1 MiB)`: over-limit is `StreamTooLong`,
/// other failures carry their Zig error name.
fn read_manifest(path: &str) -> Result<Vec<u8>, &'static str> {
    let meta = std::fs::metadata(path).map_err(|e| zig_error_name(&e))?;
    if !meta.is_file() {
        return Err("IsDir");
    }
    if meta.len() > 1024 * 1024 {
        return Err("StreamTooLong");
    }
    std::fs::read(path).map_err(|e| zig_error_name(&e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verify_args(path: &str) -> Vec<Vec<u8>> {
        vec![
            b"sa".to_vec(),
            b"sla".to_vec(),
            b"stability".to_vec(),
            b"verify".to_vec(),
            path.as_bytes().to_vec(),
        ]
    }

    #[test]
    fn schema_output_shape() {
        let args: Vec<Vec<u8>> = vec![
            b"sa".to_vec(),
            b"sla".to_vec(),
            b"stability".to_vec(),
            b"schema".to_vec(),
        ];
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let rc = run_stability_command(&args, 3, &mut out, &mut err, false).unwrap();
        assert_eq!(rc, 0);
        assert!(err.is_empty());
        // Golden: sha256(out) ==
        // 99bf7d0459f3ee93ace446c571e7075265daf6fe27d1ba797e26f88f934a4d11
        // (asserted live on the venue; here: shape anchors).
        assert!(out.starts_with(b"\n{\n  \"$schema\":"));
        assert!(out.ends_with(b"}\n\n"));
        assert_eq!(out.len(), SCHEMA_JSON.len() + 1);
    }

    #[test]
    fn verify_missing_file_is_code_1() {
        let args = verify_args("/definitely/not/a/real/manifest.json");
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let rc = run_stability_command(&args, 3, &mut out, &mut err, false).unwrap();
        assert_eq!(rc, 1);
        let err = String::from_utf8(err).unwrap();
        assert!(err.contains("File Error: failed to read /definitely/not/a/real/manifest.json: error.FileNotFound"));
    }

    #[test]
    fn verify_without_path_reports_missing() {
        let args: Vec<Vec<u8>> = vec![
            b"sa".to_vec(),
            b"sla".to_vec(),
            b"stability".to_vec(),
            b"verify".to_vec(),
        ];
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let rc = run_stability_command(&args, 3, &mut out, &mut err, false).unwrap();
        assert_eq!(rc, 1);
        assert!(String::from_utf8(err)
            .unwrap()
            .starts_with("Missing stability manifest path\nusage: sa sla stability verify"));
    }
}
