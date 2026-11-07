# Divergences — A0 command surface (INT-2 lane, phase 3)

Entries A0-1..A0-4, registered by the A0 slice (descriptor
`handle_command` for the no-pipeline commands). Reference: venue
head 6002b386 sources (plugin.zig / plugin_commands.zig /
plugin_cli.zig / plugin_skills.zig / stability_metadata.zig) plus
live probes against the installed Zig plugin and the Rust cdylib
through the same host (sa 0.1.8), 2026-11-07.

## A0-1 — Host intercepts every help form; the plugins' help branches are unreachable through `sa sla` (observation; parity-method consequence)

For ANY invocation of the form `sa sla ... -h|--help ...` (any
position, any subcommand) and for `sa sla help`, the host
(sci 0.1.8 CLI shim for the deprecated `sa sla` dispatch form)
prints the plugin's **sap.json `help` field** to stdout itself and
returns rc 0 WITHOUT calling `handle_command`. Proof: the manifest
help string is exactly 1022 bytes, and `sla sab --help`,
`sla check --help`, `sla build x.sla --help` (a form where the
plugin's own parser would stop at the source file and never see
the flag), `sla stability verify --help`, `sla help`, and
`sla help extra` all produce exactly those 1022 bytes
(sha256 74b4b5a32fea0d03…) on stdout with empty stderr, on the Zig
plugin. The plugins' own help writers (Zig `writeCommandHelp` and
the mirrored Rust `cli_help`) are exercised only via error paths
that append usage text to stderr (unknown option / missing
argument family) and via unit tests. A0 parity therefore counts
--help cases as host-vs-host evidence, and covers the plugin-level
help text through those error paths (skills --bogus,
verify-without-path, init extra argument, schema --bogus — all
byte-equal live). Classification: host behavior, faithfully left
as-is (no port action); recorded so nobody later "fixes" the
plugin help routing against this observation.

## A0-2 — stability verify: std.json parse-error names collapse to two (mirrored; residual unprobed shapes)

Zig formats the JSON parse failure as `invalid JSON: {@errorName}`
over std.json's scanner. Fourteen malformed manifests probed
against the real plugin (bad token start, bad escape, raw control
char, leading-zero number, trailing comma, single quotes,
unquoted key, misspelled literal `nul`, truncated object/string,
bad \u hex, lone surrogate, empty input, lone `{`, trailing
garbage) surface exactly TWO names: `UnexpectedEndOfInput` when
the input ends mid-value, `SyntaxError` for everything else. The
Rust hand parser keeps finer internal classes and collapses them
onto this two-name mapping at the report boundary; all fourteen
shapes are byte-equal live (five of them are in the standing
parity matrix). Residual: shapes outside the probed set (e.g.
input ending inside a `\u` escape or after a number's `-`) follow
the same end-of-input rule in the port but were not individually
probed. Classification: intentional mirror of observed behavior;
residual risk noted, not hidden.

## A0-3 — out_code on the Failed path: stub wrote 1, Zig writes 0 (port defect, fixed in this slice)

The phase-1 stub set `*out_code = 1` before returning
AbiStatus::Failed. plugin.zig's `runSlaCommandAbi` initializes
out_code to 0 and only overwrites it with a command's return code
on the Ok path, so a Failed (or UnknownCommand) outcome leaves 0.
The A0 rewiring follows plugin.zig exactly. Classification: port
defect in the pre-A0 stub, corrected; flagged here because it is
an outward behavior change relative to the phase-1/2 cdylib.

## A0-4 — std.fs.path.join keeps a leading `./` (mirrored Zig quirk)

`join(".", "src", "main.sla") == "./src/main.sla"` (printed by
`init`'s Entry line), while interior `.` segments drop and `..`
collapses lexically. Proven live: `sa sla init` with the default
root prints `Entry: ./src/main.sla` on both implementations.
`cmdio::zig_join` mirrors the rule. Classification: intentional
mirror (Zig std behavior), not a defect on either side.
