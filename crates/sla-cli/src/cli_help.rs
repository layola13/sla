//! `commandUsage` / `writeCommandHelp`, transcribed from
//! `plugin_cli.zig` (venue head 2cc12ea8) — the per-command help
//! texts written to stderr by every command's `-h/--help` path and
//! by option-parse failures. Byte-exact by construction against
//! the Zig source; note the usage strings still say `sa sla ...`
//! because that is what the original prints (the standalone binary
//! mirrors the text, it does not rebrand it).

use std::io::Write;

/// Zig `commandUsage`.
pub fn command_usage(command: &str) -> &'static str {
    match command {
        "init" => "usage: sa sla init [path]\n",
        "skills" => "usage: sa sla skills [--json]\n",
        "stability" => "usage: sa sla stability <schema|verify> [options]\n",
        "stability schema" => "usage: sa sla stability schema [--json]\n",
        "stability verify" => "usage: sa sla stability verify <manifest.json> [--json]\n",
        "build" => "usage: sa sla build [file] [-p <package>] [--out <file>]\n",
        "build-workspace" => "usage: sa sla build-workspace [-p <package>] [sa-build-exe-options...]\n",
        "build-exe" => "usage: sa sla build-exe [file] [-p <package>] [sa-build-exe-options...]\n",
        "build-wasm" => "usage: sa sla build-wasm [file] [-p <package>] [sa-build-wasm-options...]\n",
        "build-js" => "usage: sa sla build-js [file] [-p <package>] [sa-build-js-options...]\n",
        "build-c" => "usage: sa sla build-c [file] [-p <package>] [sa-build-c-options...]\n",
        "sab" => "usage: sa sla sab <build|workspace|disasm> [options]\n       sa slab <build|workspace|disasm> [options]\n",
        "sab build" => "usage: sa sla sab build [file] [-p <package>] [--out <file.sab>]\n       sa slab build [file] [-p <package>] [--out <file.sab>]\n",
        "sab workspace" => "usage: sa sla sab workspace [-p <package>] [--sab-out <file.sab>] [sa-build-exe-options...]\n       sa slab workspace [-p <package>] [--sab-out <file.sab>] [sa-build-exe-options...]\n",
        "sab disasm" => "usage: sa sla sab disasm <file.sab> [--out <file.sa>]\n       sa slab disasm <file.sab> [--out <file.sa>]\n",
        "check" => "usage: sa sla check [file] [-p <package>]\n",
        "test" => "usage: sa sla test [file] [-p <package>] [--test-backend auto|sab|sa] [sa-test-options...]\n",
        _ => "usage: sla <command> [options]\n       sa sla <command> [options]\n",
    }
}

/// Zig `writeCommandHelp`.
pub fn write_command_help(writer: &mut impl Write, command: &str) -> std::io::Result<()> {
    writer.write_all(command_usage(command).as_bytes())?;
    match command {
        "init" => {
            writer.write_all(b"\n")?;
            writer.write_all(b"Create a new SLA binary project with sa.mod, src/main.sla, and .gitignore.\n\n")?;
            writer.write_all(b"  -h, --help              Show this help message\n")?;
        }
        "skills" => {
            writer.write_all(b"\n")?;
            writer.write_all(b"List SLA plugin capabilities. Text mode also writes agent skills into the current directory.\n\n")?;
            writer.write_all(b"  --json                  Emit machine-readable capability JSON\n")?;
            writer.write_all(b"  -h, --help              Show this help message\n")?;
        }
        "stability" => {
            writer.write_all(b"\n")?;
            writer.write_all(b"Validate downstream stability metadata manifests without assigning downstream label meaning.\n\n")?;
            writer.write_all(b"  schema                  Emit the JSON schema for stability metadata\n")?;
            writer.write_all(b"  verify <manifest.json>  Validate a downstream manifest\n")?;
            writer.write_all(b"  -h, --help              Show this help message\n")?;
        }
        "stability schema" => {
            writer.write_all(b"\n")?;
            writer.write_all(b"  --json                  Accepted for consistency; schema output is JSON\n")?;
            writer.write_all(b"  -h, --help              Show this help message\n")?;
        }
        "stability verify" => {
            writer.write_all(b"\n")?;
            writer.write_all(b"  --json                  Emit machine-readable verification output\n")?;
            writer.write_all(b"  -h, --help              Show this help message\n")?;
        }
        "build" | "build-workspace" | "build-exe" | "build-wasm" | "build-js" | "build-c"
        | "sab build" | "sab workspace" | "check" | "test" => {
            writer.write_all(b"\n")?;
            writer.write_all(b"  -p, --package <name>    Select a workspace member package\n")?;
            if matches!(
                command,
                "build-exe" | "build-wasm" | "build-js" | "build-c" | "build-workspace" | "test"
            ) {
                writer.write_all(b"  --emit-sab              Also write a sibling .sab artifact for inspection\n")?;
            }
            if command == "test" {
                writer.write_all(b"  --test-backend auto|sab|sa\n")?;
                writer.write_all(b"                          Select test compiler backend; default auto uses SAB\n")?;
            }
            if command == "sab build" {
                writer.write_all(b"  -o, --out <file.sab>    Also write SAB output file; default uses .sla-cache/sab/\n")?;
            }
            if command == "sab workspace" {
                writer.write_all(b"  --sab-out <file.sab>    Also write SAB output file; default uses .sla-cache/sab/\n")?;
                writer.write_all(b"  --emit-sab              Also write a sibling .sab artifact for inspection\n")?;
            }
            writer.write_all(b"  -h, --help              Show this help message\n")?;
        }
        "sab disasm" => {
            writer.write_all(b"\n")?;
            writer.write_all(b"  -o, --out <file.sa>     Write text SA debug output instead of stdout\n")?;
            writer.write_all(b"  -h, --help              Show this help message\n")?;
        }
        _ => {}
    }
    Ok(())
}

/// The top-level `help` command body (plugin_commands.zig writes
/// it line by line to stderr).
pub const HELP_TEXT: &str = "usage: sla <command> [options]\n       sa sla <command> [options]\n\nCommands:\n  init       [path]\n  skills     [--json]\n  stability  schema|verify ...\n  build      [file] [-p <package>] [--out <file>]\n  build-workspace [-p <package>] [sa-build-exe args]\n  build-exe  [file] [-p <package>] [sa-build-exe args]\n  build-wasm [file] [-p <package>] [sa-build-wasm args]\n  build-js   [file] [-p <package>] [sa-build-js args]\n  build-c    [file] [-p <package>] [sa-build-c args]\n  sab build  [file] [-p <package>] [--out <file.sab>]\n  sab workspace [-p <package>] [--sab-out <file.sab>] [sa-build-exe args]\n  sab disasm <file.sab> [--out <file.sa>]\n  slab build|workspace|disasm ...    Short alias\n  check      [file] [-p <package>]\n  test       [file] [-p <package>] [--test-backend auto|sab|sa] [sa-test args]\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_and_help_shapes() {
        assert_eq!(
            command_usage("build"),
            "usage: sa sla build [file] [-p <package>] [--out <file>]\n"
        );
        let mut buf: Vec<u8> = Vec::new();
        write_command_help(&mut buf, "test").unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("--test-backend auto|sab|sa"));
        assert!(text.contains("--emit-sab"));
        let mut buf: Vec<u8> = Vec::new();
        write_command_help(&mut buf, "sab disasm").unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("-o, --out <file.sa>"));
        assert!(!text.contains("--package"));
    }
}
