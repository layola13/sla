//! `commandUsage` / `writeCommandHelp`, transcribed from
//! `plugin_cli.zig` (venue head 6002b386) — the per-command help
//! texts written to stderr by every command's `-h/--help` path and
//! by option-parse failures. Byte-exact by construction; the venue
//! live-parity matrix (docs/phase3/int2.md A0 section) samples
//! these against the Zig plugin.

use crate::cmdio::CmdWrite;

/// Zig `commandUsage`.
pub fn command_usage(command: &str) -> &'static str {
    match command {
        "init" => "usage: sa sla init [path]\n",
        "skills" => "usage: sa sla skills [--json]\n",
        "stability" => "usage: sa sla stability <schema|verify> [options]\n",
        "stability schema" => "usage: sa sla stability schema [--json]\n",
        "stability verify" => "usage: sa sla stability verify <manifest.json> [--json]\n",
        "build" => "usage: sa sla build [file] [-p <package>] [--out <file>]\n",
        "build-workspace" => {
            "usage: sa sla build-workspace [-p <package>] [sa-build-exe-options...]\n"
        }
        "build-exe" => "usage: sa sla build-exe [file] [-p <package>] [sa-build-exe-options...]\n",
        "build-wasm" => {
            "usage: sa sla build-wasm [file] [-p <package>] [sa-build-wasm-options...]\n"
        }
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

const PACKAGE_LINE: &str = "  -p, --package <name>    Select a workspace member package\n";
const EMIT_SAB_LINE: &str = "  --emit-sab              Also write a sibling .sab artifact for inspection\n";
const HELP_LINE: &str = "  -h, --help              Show this help message\n";

/// Zig `writeCommandHelp`: usage line plus the command's body.
pub fn write_command_help(w: &mut impl CmdWrite, command: &str) -> Result<(), ()> {
    w.write_str(command_usage(command))?;
    match command {
        "init" => {
            w.write_str("\n")?;
            w.write_str("Create a new SLA binary project with sa.mod, src/main.sla, and .gitignore.\n\n")?;
            w.write_str(HELP_LINE)?;
        }
        "skills" => {
            w.write_str("\n")?;
            w.write_str("List SLA plugin capabilities. Text mode also writes agent skills into the current directory.\n\n")?;
            w.write_str("  --json                  Emit machine-readable capability JSON\n")?;
            w.write_str(HELP_LINE)?;
        }
        "stability" => {
            w.write_str("\n")?;
            w.write_str("Validate downstream stability metadata manifests without assigning downstream label meaning.\n\n")?;
            w.write_str("  schema                  Emit the JSON schema for stability metadata\n")?;
            w.write_str("  verify <manifest.json>  Validate a downstream manifest\n")?;
            w.write_str(HELP_LINE)?;
        }
        "stability schema" => {
            w.write_str("\n")?;
            w.write_str("  --json                  Accepted for consistency; schema output is JSON\n")?;
            w.write_str(HELP_LINE)?;
        }
        "stability verify" => {
            w.write_str("\n")?;
            w.write_str("  --json                  Emit machine-readable verification output\n")?;
            w.write_str(HELP_LINE)?;
        }
        "build" | "build-workspace" | "build-exe" | "build-wasm" | "build-js" | "build-c"
        | "sab build" | "sab workspace" | "check" | "test" => {
            w.write_str("\n")?;
            w.write_str(PACKAGE_LINE)?;
            if matches!(
                command,
                "build-exe" | "build-wasm" | "build-js" | "build-c" | "build-workspace" | "test"
            ) {
                w.write_str(EMIT_SAB_LINE)?;
            }
            if command == "test" {
                w.write_str("  --test-backend auto|sab|sa\n")?;
                w.write_str("                          Select test compiler backend; default auto uses SAB\n")?;
            }
            if command == "sab build" {
                w.write_str("  -o, --out <file.sab>    Also write SAB output file; default uses .sla-cache/sab/\n")?;
            }
            if command == "sab workspace" {
                w.write_str("  --sab-out <file.sab>    Also write SAB output file; default uses .sla-cache/sab/\n")?;
                w.write_str(EMIT_SAB_LINE)?;
            }
            w.write_str(HELP_LINE)?;
        }
        "sab disasm" => {
            w.write_str("\n")?;
            w.write_str("  -o, --out <file.sa>     Write text SA debug output instead of stdout\n")?;
            w.write_str(HELP_LINE)?;
        }
        _ => {}
    }
    Ok(())
}
