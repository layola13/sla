# Phase 3 — INT-2 contract segment (delivery report)

Lane: INT-2. Venue: Kaggle venue 3 (hop 2202), tree
/root/sla-plugin-rust synced per-path with md5 verification
(28/28 files OK). Zig reference: venue tree head 6002b386 +
local tree ~/workspace/local-sa/sa_plugin_sla (read-only).

## 1. What was built (ruling D-A2 placement)

| piece | home | mirrors |
|---|---|---|
| `.sai`/`.sal` contract parsing -> owned contract declarations | `crates/sla-parser/src/contract.rs` (340 lines) | contract_parser.zig (223 lines), incl. its unit test ported assertion-for-assertion |
| Import path resolution (sa_std / sla_std / plugin / relative / glob, ancestor roots + env roots) | `crates/sla-parser/src/imports.rs` (512 lines) | plugin_imports.zig (446 lines) |
| Recursive contract loading (visited set, scan-before-load, `.sai`->externs, `.sal`->defines) | `crates/sla-parser/src/contract_load.rs` (326 lines) | plugin_import_expand.zig `loadImportedContracts` :1429 + `loadImportContractsRecursive` :1338 + `scanExpandedSourceImports` :91 |
| Checker-facing registration via the ONE existing entry `Checker::register_extern_func` | `crates/sla-plugin/src/contracts.rs` (123 lines) | plugin layer's `loadImportedContracts` call + type_checker.zig `loadContracts` :2105 |
| Oracle pipeline consumes the same machinery (contracts stage + `03a_contracts.txt` boundary dump) | `crates/sla-oracle/src/main.rs` (654 lines) | plugin_compile.zig ordering (load contracts ahead of checkProgram) |

Placement tension, stated plainly: D-A2 assigns the orchestration
to the sla-plugin wiring layer and requires sla-oracle to consume
THE SAME wiring. The frozen Cargo graph gives sla-oracle no edge
to sla-plugin (and this lane may not add edges), so the shared
machinery (resolution + recursion) lives in sla-parser — the only
crate both consumers already depend on — the registration wrapper
lives in sla-plugin as D-A2 prescribes, and oracle's call site is a
5-line shim performing the identical registration the plugin
wrapper performs. There is exactly ONE expansion implementation.
**Coordinator action requested:** add the `sla-oracle -> sla-plugin`
Cargo edge so oracle can call `sla_plugin::contracts` directly and
the shim can be deleted.

Out-of-slice parts of plugin_import_expand.zig (`.sla` body
inlining, module tables, reachability materialization, imported
macros) are registered as phase-remaining (INT2-4) and refuse
loudly: `sla_plugin::contracts::expand_sla_imports_unwired()`,
plus per-file macro/`.sla` listings in every load report.

## 2. The layer finding (why the fixtures still die, and whose it is)

The phase-2 blocking list assumed `println` was a std-surface call
that contract registration would unlock. Dual verification (Zig
source + venue probes) shows otherwise:

- In Zig, `println` / `str_eq` are **checker name-specials**
  (type_checker.zig isPrintlnCall / isStrEqCall arms; any-arity
  void / typed boolean), needing no import and no registration.
  Venue: a no-import file using println with 1/2/3 args and
  str_eq checks rc=0; `str_eq(1, 2)` is rejected rc=1 TypeMismatch
  (docs/probes/int2/ZIG_PROBES.md).
- The Rust checker has no such arms (INT2-1, a checker-lane
  mirroring gap in the same family as INT-1). They cannot be
  expressed through `register_extern_func` without forging
  verdicts (fixed arity; untyped bypass would accept
  `str_eq(1, 2)`). Per D-A2 ("registration surface insufficient ->
  stop and register"), this lane did NOT fake them. The blocker is
  registered as INT2-1 for the coordinator to sequence to the
  checker lane; a plugin test pins the exact current diagnostic.

What the contract segment itself provably does now: direct calls
to real contract externs pass. Venue probe p3
(`sa_print_bytes("hi", 2)` with print.sai imported) went from
check-rejected (phase 2) to **full pipeline rc=0**, emitting text
codegen output (sha256
b6927af4ba5fea751801ee5ca77a95a61d02032f51e75be8c2fc410eedc09c52;
no Zig golden exists for p3, so no byte-parity is claimed). The
Zig plugin accepts the same probe (rc=0).

## 3. Per-fixture accounting (oracle, build mode, venue)

Environment: `SA_STD_DIR=/content/sa_all/sci/sa_std`. Death point
moved from "check, empty extern registry" (phase 2) to "check,
solely the missing println arm" for every fixture.

| fixture | visited files | externs registered | layout defines carried (INT2-3) | macro files reported (INT2-4) | death point |
|---|---|---|---|---|---|
| 01_hello | 30 | 56 | 323 | 13 | check: `checkExpr failed at call println with 1 args` (rc=5) |
| 02_arith | 2 | 16 | 0 | 0 | check: `... println with 2 args` (rc=5) |
| 03_if_else | 2 | 16 | 0 | 0 | check: `... println with 2 args` (rc=5) |
| 04_while_sum | 2 | 16 | 0 | 0 | check: `... println with 2 args` (rc=5) |
| 07_functions | 2 | 16 | 0 | 0 | check: `... println with 2 args` (rc=5) |

Notes:
- Fixture 01's closure is the full string.sa tree (30 files:
  print.sai, fmt.sai, string.sa, core/slice.{sal,sa}, char.{sa,sal},
  ascii.sa, core/ascii.{sal,sa}, vec.{sa,sal}, alloc/vec.{sa,sal},
  hash.{sa,sal}, string_format.sa, string.sai, io.sai, ptr.{sal,sa},
  core/{iter.sa,rc.sal,arc.sal}, ...). Nested registration order is
  Zig's scan-before-load order.
- `str_eq` (fixture 01): NOT an independent blocker in build mode
  (the @test body is pruned before check); in --no-build-mode the
  run still dies at `main`'s println first (verified on venue). It
  is the latent next death behind the println arm, in full mode
  only. The phase-2 description "01 另有 str_eq 第二挡" is refined
  accordingly.
- No fixture reaches lowering/codegen through the oracle yet —
  that is gated on INT2-1 alone. The codegen-text lane's own
  hand-fed parity harness remains 5/5 (untouched by this lane).
- Regression control: oracle e2e_b / e2e_c outputs unchanged,
  sha256 8623e8d0… / fe190281… — byte-identical to the phase-2
  record.

## 4. Tests (venue logs /tmp/int2_test1.log, /tmp/int2_test_full.log)

- Targeted: `cargo test -p sla-parser -p sla-plugin -p sla-oracle
  --no-fail-fast` — all green. New tests: parser lib +13
  (contract 6 incl. the ported Zig test, imports 3, contract_load 4),
  plugin lib +2, plugin integration `contracts_pipeline` +3
  (extern-call control/experiment pair, the println pinning test,
  nested-closure order) = **+18**.
- Workspace: `cargo test --workspace --no-fail-fast` = 181 passed /
  1 failed. The 181 includes the parallel SAB-C / CHK3 lanes' new
  tests landed in the shared venue tree (baseline 137 + INT-2's 18
  + theirs). **The 1 failure is not this lane's:**
  `sla-codegen-sab driver_real_product_array_const_parity`
  (197 vs 225 bytes). sla-codegen-sab depends only on sla-ast +
  sla-lowering — it cannot see any INT-2 change — and its venue
  sources carry the SAB-C lane's mid-flight C-group syncs
  (prescan_consts.rs / prescan_bindings.rs / symbols.rs, timestamps
  02:22-02:26 UTC). Attribution left to the SAB-C lane's own
  accounting; flagged here so the coordinator does not read it as
  an INT-2 regression.
- File-line discipline: all files this lane touched are <=700
  lines (largest: sla-oracle/src/main.rs 654).

## 5. Divergences and feedback

- New divergence file: docs/divergences/contracts.md, INT2-1 ..
  INT2-8. Headline: INT2-1 (checker missing name-special arms —
  port defect, checker lane), INT2-2 / INT2-3 (registration-surface
  gaps: return_fallible, layout_defines), INT2-4 / INT2-5 / INT2-7
  (phase-remaining coverage), INT2-6 (D-C3 omission), INT2-8
  (mirrored quirks). A0 added docs/divergences/a0-commands.md,
  A0-1 .. A0-4 (see §8).
- Feedback slots FB-021..FB-024: **none issued** — no new Zig-side
  defect candidate met the bar on this lane; slots returned unused.

## 6. lib.rs changes by this lane (per coordination note)

- `crates/sla-parser/src/lib.rs`: added `pub mod contract;`,
  `pub mod contract_load;`, `pub mod imports;` and re-exports
  (`ExternalFunction`, `LayoutDefine`, `load_imported_contracts`,
  `ContractLoadError`, `LoadReport`, `ResolvedImport`); module-map
  doc comment extended.
- `crates/sla-plugin/src/lib.rs`: added `pub mod contracts;`.
  A0 (§8) further added `pub mod cli_help;`, `pub mod cmdio;`,
  `pub mod commands;`, `pub mod init_cmd;`, `pub mod skills_cmd;`,
  `pub mod stability;`, `pub mod stability_validate;`, rewired
  `run_sla_command_abi` from the loud-Failed stub onto
  `commands::dispatch`, and rewrote the crate doc comment to match.
- `crates/sla-oracle`: no lib.rs (bin crate); src/main.rs gained
  the contracts stage + two helpers
  (`sla_plugin_contract_paths`, `contracts_dump_text`).
- No Cargo.toml was touched; no dependency edge was added.

## 7. Remaining items

1. INT2-1 -> checker lane: add the name-special call arms
   (println / str_eq / len / format / hash / debug, in Zig's
   dispatch position) — unblocks all 5 fixtures' check segment;
   then re-run this accounting (expect death to move to
   lowering/codegen or full parity).
2. Coordinator: rule on the `sla-oracle -> sla-plugin` Cargo edge
   (delete the oracle shim), and on INT2-2 / INT2-3 registration
   surface (ExternSig fallible field; a layout_defines table).
3. Phase-remaining in this segment: imported macros (INT2-4),
   workspace-package resolution (INT2-5), `@expand_tuple`
   expansion (INT2-7), `.sla` body expansion (out of slice).

## 8. A0 slice (coordinator add-on, lower priority) — COMPLETED

Scope per handler-eval §5.2 batch 1: the descriptor
`handle_command` surface for the commands that do not drive the
compile pipeline — `help`, `skills [--json]`, `stability
schema|verify`, `init`, the unknown-command paths, the `sab`
dispatcher's help/error paths, and every command's option-parse /
help-error paths. Pipeline commands still end in a loud
AbiStatus::Failed after their options parse (their pipelines are
other lanes' work); nothing was silently dropped.

### What was built (all in crates/sla-plugin/src/)

- `commands.rs` — `runSlaCommandImpl` dispatch mirror
  (plugin_commands.zig): top-level dispatch, `parseSlaCliOptionsFrom`
  (incl. the test-only `--test-backend` handling and `-p/--package`
  forms), the pipeline commands' options/help shim, the `sab`
  sub-dispatcher (help/error paths real; build/workspace/disasm
  stay loud-Failed past option parsing).
- `cli_help.rs` — `writeCommandHelp` for every command, transcribed
  from plugin_cli.zig (venue head).
- `skills_cmd.rs` — skills JSON writer (escaping per
  plugin_skills.zig), text mode, and the agent-skill markdown
  (both SKILL.md files), static table shared with lib.rs's
  descriptor statics.
- `stability.rs` + `stability_validate.rs` — the schema constant
  (extracted mechanically from stability_metadata.zig), verify's
  option loop, both report writers, and a hand JSON parser +
  validator mirroring `validateManifestText` statement-for-statement
  (incl. which checks `continue` past the counters). Hand parser
  because the workspace carries no JSON dependency and Cargo edges
  are the coordinator's (handler-eval §4.3 anticipated this).
- `init_cmd.rs` — init scaffolding with exclusive-create per-file
  error lines; `cmdio.rs` — the CmdWrite sink trait (host streams /
  Vec capture), Zig error-name table, `zig_join` / `zig_basename`.
- `lib.rs::run_sla_command_abi` — argv decoding, ctx.json_mode,
  (status, out_code) mapping exactly as plugin.zig (see A0-3).

### Live parity (same window, venue, sa 0.1.8 host)

Twin sandboxes (identical relative layout), Zig plugin from its
installed home (`/root/.local/share/sa_plugins/installed/sla/
current`, md5-verified == zig-out build at venue head) vs the Rust
cdylib staged into a twin HOME at the same installed path with the
same sap.json; SA_PLUGIN_DEV=1, SA_STD_DIR pinned,
SLA_NO_FRESHNESS_CACHE=1. Compared per case: rc, stdout bytes,
stderr bytes (host `note[SA-PLUGIN]:` deprecation lines stripped —
host-produced on both sides), and the produced file tree
(relative paths + sha256).

- Symbol gate (handler-eval §2.1 gate 0): `nm -D --defined-only`
  sets EQUAL — both libraries export exactly
  `saasm_plugin_descriptor_v1` + `saasm_plugin_descriptor_v1_fn`.
- **Matrix: 33 / 33 PASS, zero diffs.** Cases: help, no-subcommand,
  skills --json, skills (text), skills -h, stability schema
  (± --json), verify {valid × text, valid × JSON, validation-
  invalid, missing file, 6 malformed-JSON shapes}, init {new,
  repeat, default root, extra argument, unknown option},
  unknown command, check --help, build --help, sab {no args,
  unknown via slab, disasm no args}, stability {no args, unknown
  subcommand}, skills --bogus, verify without path, schema
  --bogus.
- Goldens reproduced in-window: skills --json stdout sha256
  `efbe756e11fc8e6c0985c9c0aefd0a6011788229883bc7c62da86d377fef696d`;
  stability schema stdout sha256
  `99bf7d0459f3ee93ace446c571e7075265daf6fe27d1ba797e26f88f934a4d11`.
- Artifact shas (identical on both sides): codex SKILL.md
  `e5df2224086ec89f2d47d92ed72a9b37e2b7d2229adec65a2a419e3e910bb2f9`,
  claude SKILL.md
  `9e796b84cc4161ffd908a6a3ee8c4ffc2bbf174e295c4a431995bd3ae3093385`,
  init tree sa.mod
  `c55f15834fbded88daedf9246d06ac45d146406c82026c9cbb748b4c3d4b62b1`,
  src/main.sla
  `c6dae32074185575bc80004577189e8f933b850738ec41087ace4c8eeb8989f4`,
  .gitignore
  `253b9b97240842ebf7cd34a8aa6576303789e0da7c0ca882d0494cfc543450cb`.
- Method note (A0-1): every `-h/--help`/`help` form is intercepted
  by the HOST, which prints the sap.json help field (1022 bytes)
  itself; those matrix cases are host-vs-host. The plugins' own
  help writers are covered by the error-path cases above (help
  text appended on stderr, byte-equal) plus unit tests.
- First run was 20/24; the four diffs were real and are fixed:
  Zig's `{}` error formatting carries the `error.` prefix (verify
  read errors, init create errors), the two-name JSON error
  mapping (A0-2), and join's leading `./` (A0-4). A later 8-case
  "failure" batch was a harness artifact (the tree snapshot hashed
  itself); the snapshot now lives outside the sandbox, and the
  final run is clean 33/33.
- Unit tests: sla-plugin lib 21 passed (incl. golden-byte skills
  JSON, the ported stability_metadata Zig tests, init scaffold +
  repeat, dispatch paths) + 3 integration. Divergences registered:
  docs/divergences/a0-commands.md (A0-1..A0-4). FB-021..024 still
  unused.

## 9. Cross-line notes

- Final gate re-run (after A0 + doc sync, venue): the three
  lane crates are all green (51 tests across sla-parser /
  sla-plugin / sla-oracle targets), and the full workspace now
  reads **226 passed / 0 failed** — the single SAB failure
  recorded in §4 cleared once the SAB-C lane's in-flight sync
  settled, confirming the §4 attribution (not an INT-2
  regression). All 37 lane files (both crates + oracle main +
  docs/probes) verified hash-identical local vs venue.
- **D-C18 (CHK3 lane) — aligned.** The pin in
  `crates/sla-plugin/tests/contracts_pipeline.rs`
  (`println_remains_blocked_by_missing_checker_builtin_arm`) was
  updated from the pre-alignment synthesized fallback text to the
  D-C18 wording: the diagnostic must contain
  `Undefined call: println` (Zig's three unresolved-call shapes
  all read `Undefined call: {name}` + UndefinedVariable, per
  CHK3's 31 real-plugin probes). The test's intent is unchanged —
  println remains blocked by the missing checker name-special arm
  (INT2-1) — and the updated pin passes on the venue tree with
  CHK3's alignment landed (cargo test -p sla-plugin, 3/3
  integration green).
