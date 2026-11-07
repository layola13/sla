//! The `skills` command, mirrored from `plugin_skills.zig` +
//! `runSlaSkillsCommand` (plugin_cli.zig), venue head 6002b386.
//!
//! JSON mode is a pure function of the static skills table (venue
//! golden sha256 efbe756e11fc8e6c… — reproduced by construction
//! and asserted in the tests below). Text mode additionally writes
//! the two agent SKILL.md files into the current directory, in
//! Zig's order (files first, stdout after), and file failures fail
//! the whole invocation (the Zig error union path).

use crate::cli_help::write_command_help;
use crate::cmdio::CmdWrite;

/// The skills table as plain data (the descriptor's statics in
/// lib.rs carry the same strings for the ABI surface).
pub const SKILL_SECTIONS: &[(&str, &str, &[&str])] = &[(
    "sla",
    "Sla compiler and tools",
    &[
        "sla init [path]",
        "sla skills [--json]",
        "sla stability schema|verify ...",
        "sla build [file] [-p <package>] [--out <file>]",
        "sla build-workspace [-p <package>] [sa-build-exe-options...]",
        "sla build-exe [file] [-p <package>] [sa-build-exe-options...]",
        "sla build-wasm [file] [-p <package>] [sa-build-wasm-options...]",
        "sla build-js [file] [-p <package>] [sa-build-js-options...]",
        "sla build-c [file] [-p <package>] [sa-build-c-options...]",
        "sla sab build [file] [-p <package>] [--out <file.sab>]",
        "sla sab workspace [-p <package>] [--sab-out <file.sab>] [sa-build-exe-options...]",
        "slab build|workspace|disasm ...",
        "sla check [file] [-p <package>]",
        "sla test [file] [-p <package>] [--test-backend auto|sab|sa] [sa-test-options...]",
    ],
)];

/// Zig `writeJsonString` (plugin_skills variant): control bytes
/// become `\uXXXX` with UPPERCASE hex, minimum 4 digits.
pub fn write_json_string(w: &mut impl CmdWrite, text: &[u8]) -> Result<(), ()> {
    w.write_all(b"\"")?;
    for &c in text {
        match c {
            b'"' => w.write_all(b"\\\"")?,
            b'\\' => w.write_all(b"\\\\")?,
            b'\n' => w.write_all(b"\\n")?,
            b'\r' => w.write_all(b"\\r")?,
            b'\t' => w.write_all(b"\\t")?,
            0x00..=0x1f => w.print(format_args!("\\u{:04X}", c))?,
            _ => w.write_all(&[c])?,
        }
    }
    w.write_all(b"\"")
}

fn write_json_string_array(w: &mut impl CmdWrite, items: &[&str]) -> Result<(), ()> {
    w.write_all(b"[")?;
    for (idx, item) in items.iter().enumerate() {
        if idx != 0 {
            w.write_all(b",")?;
        }
        write_json_string(w, item.as_bytes())?;
    }
    w.write_all(b"]")
}

/// Zig `writeSlaSkillsJson`.
pub fn write_skills_json(w: &mut impl CmdWrite) -> Result<(), ()> {
    w.write_str("{\"status\":\"ok\",\"skills\":[")?;
    for (idx, (name, summary, items)) in SKILL_SECTIONS.iter().enumerate() {
        if idx != 0 {
            w.write_all(b",")?;
        }
        w.write_all(b"{")?;
        w.write_str("\"name\":")?;
        write_json_string(w, name.as_bytes())?;
        w.write_str(",\"summary\":")?;
        write_json_string(w, summary.as_bytes())?;
        w.write_str(",\"items\":")?;
        write_json_string_array(w, items)?;
        w.write_all(b"}")?;
    }
    w.write_str("]}\n")
}

/// Zig `writeSlaSkillSectionText`.
fn write_section_text(w: &mut impl CmdWrite, name: &str, summary: &str, items: &[&str]) -> Result<(), ()> {
    w.print(format_args!("{name}\n"))?;
    w.print(format_args!("summary: {summary}\n"))?;
    for item in items {
        w.print(format_args!("- {item}\n"))?;
    }
    Ok(())
}

/// Zig `writeSlaAgentSkillMarkdown`.
fn agent_skill_markdown(agent_name: &str) -> String {
    let description = if agent_name == "claude" {
        "Use the installed SLA plugin from Claude to build, check, test, scaffold, and inspect direct SAB workflows."
    } else {
        "Use the installed SLA plugin from Codex to build, check, test, scaffold, and inspect direct SAB workflows."
    };
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("name: \"sla\"\n");
    out.push_str("description: ");
    {
        let mut buf: Vec<u8> = Vec::new();
        // The description is plain ASCII; the JSON writer cannot
        // fail on a Vec sink.
        let _ = write_json_string(&mut buf, description.as_bytes());
        out.push_str(std::str::from_utf8(&buf).unwrap_or("\"\""));
    }
    out.push('\n');
    out.push_str("when_to_use: \"Use when working on .sla sources, SLA workspace builds, direct SLA-to-SAB output, or SLA plugin CLI commands.\"\n");
    out.push_str("---\n\n");
    out.push_str("# SLA Toolchain\n\n");
    out.push_str("## Core Workflow\n");
    out.push_str("- Use `sa sla init [path]` to scaffold a minimal SLA binary project.\n");
    out.push_str("- Use `sa sla build <file>` only when a visible `.sa` text artifact is needed.\n");
    out.push_str("- Use `sa sla build-exe <file>` or `sa sla sab workspace` for executable builds through the direct SAB path.\n");
    out.push_str("- Use `sa sla build-wasm <file>` for WebAssembly (.wasm), `sa sla build-js <file>` for JavaScript (.js), and `sa sla build-c <file>` for portable C (.c) through the same direct SAB path.\n");
    out.push_str("- Use `sa sla test <file>` for tests through the direct SAB path by default; add `--test-backend sa` only when debugging legacy `.test.sa` output.\n");
    out.push_str("- Use `sa sla sab build <file>` to emit managed SAB under `.sla-cache/sab/`; add `--out <file.sab>` only for an inspection copy.\n");
    out.push_str("- Keep SLA-to-SA and SLA-to-SAB as separate mainlines; SAB output must not be implemented as `sla -> sa -> sab`.\n");
    out.push_str("- Prefer focused checks with `timeout 120s`; do not run full test suites unless explicitly requested. Build commands do not need the timeout wrapper.\n\n");
    out.push_str("## CLI Skill Sections\n");
    for (name, summary, items) in SKILL_SECTIONS {
        out.push_str(&format!("### {name}\n"));
        out.push_str(&format!("{summary}\n"));
        for item in *items {
            out.push_str(&format!("- `{item}`\n"));
        }
        out.push('\n');
    }
    out
}

fn ensure_parent_dir(path: &str) -> std::io::Result<()> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    Ok(())
}

/// Zig `writeSlaAgentSkills`: returns (codex_path, claude_path);
/// IO failure is an invocation failure (Err), as in Zig.
pub fn write_agent_skills() -> Result<(&'static str, &'static str), ()> {
    let codex_path = ".codex/skills/sla/SKILL.md";
    let claude_path = ".claude/skills/sla/SKILL.md";
    ensure_parent_dir(codex_path).map_err(|_| ())?;
    ensure_parent_dir(claude_path).map_err(|_| ())?;
    std::fs::write(codex_path, agent_skill_markdown("codex")).map_err(|_| ())?;
    std::fs::write(claude_path, agent_skill_markdown("claude")).map_err(|_| ())?;
    Ok((codex_path, claude_path))
}

fn is_help_arg(arg: &[u8]) -> bool {
    arg == b"-h" || arg == b"--help"
}

/// Zig `runSlaSkillsCommand`. Returns the command code, or Err(())
/// when the invocation itself fails (stream/file error).
pub fn run_skills_command(
    args: &[Vec<u8>],
    option_start: usize,
    stdout: &mut impl CmdWrite,
    stderr: &mut impl CmdWrite,
    default_json_mode: bool,
) -> Result<u8, ()> {
    let mut json_mode = default_json_mode;
    let mut idx = option_start;
    while idx < args.len() {
        let arg = &args[idx];
        if is_help_arg(arg) {
            write_command_help(stderr, "skills")?;
            return Ok(0);
        }
        if arg == b"--json" {
            json_mode = true;
            idx += 1;
            continue;
        }
        stderr.print(format_args!(
            "Unknown sla skills option: {}\n",
            String::from_utf8_lossy(arg)
        ))?;
        write_command_help(stderr, "skills")?;
        return Ok(1);
    }

    if json_mode {
        write_skills_json(stdout)?;
    } else {
        let (codex, claude) = write_agent_skills()?;
        stdout.write_str("sla compiler plugin\n")?;
        stdout.print(format_args!("generated agent skills:\n- {codex}\n- {claude}\n"))?;
        for (name, summary, items) in SKILL_SECTIONS {
            write_section_text(stdout, name, summary, items)?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_json_matches_zig_golden_bytes() {
        // The exact byte stream whose sha256 is the venue golden
        // efbe756e11fc8e6c0985c9c0aefd0a6011788229883bc7c62da86d377fef696d
        // (HND evidence index; reproduced from plugin_skills.zig).
        let expected = "{\"status\":\"ok\",\"skills\":[{\"name\":\"sla\",\"summary\":\"Sla compiler and tools\",\"items\":[\"sla init [path]\",\"sla skills [--json]\",\"sla stability schema|verify ...\",\"sla build [file] [-p <package>] [--out <file>]\",\"sla build-workspace [-p <package>] [sa-build-exe-options...]\",\"sla build-exe [file] [-p <package>] [sa-build-exe-options...]\",\"sla build-wasm [file] [-p <package>] [sa-build-wasm-options...]\",\"sla build-js [file] [-p <package>] [sa-build-js-options...]\",\"sla build-c [file] [-p <package>] [sa-build-c-options...]\",\"sla sab build [file] [-p <package>] [--out <file.sab>]\",\"sla sab workspace [-p <package>] [--sab-out <file.sab>] [sa-build-exe-options...]\",\"slab build|workspace|disasm ...\",\"sla check [file] [-p <package>]\",\"sla test [file] [-p <package>] [--test-backend auto|sab|sa] [sa-test-options...]\"]}]}\n";
        let mut buf: Vec<u8> = Vec::new();
        write_skills_json(&mut buf).unwrap();
        assert_eq!(String::from_utf8(buf).unwrap(), expected);
    }

    #[test]
    fn unknown_skills_option_is_code_1_with_help() {
        let args: Vec<Vec<u8>> = vec![
            b"sa".to_vec(),
            b"sla".to_vec(),
            b"skills".to_vec(),
            b"--bogus".to_vec(),
        ];
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let rc = run_skills_command(&args, 3, &mut out, &mut err, false).unwrap();
        assert_eq!(rc, 1);
        assert!(out.is_empty());
        let err = String::from_utf8(err).unwrap();
        assert!(err.starts_with("Unknown sla skills option: --bogus\nusage: sa sla skills [--json]\n"));
    }
}
