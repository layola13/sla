# Phase-4 IMP probes — venue transcripts (summary)

Real-plugin probes against the box-installed plugin (box
sa_plugin_sla HEAD 2cc12ea8), `source /content/tools/env.sh`,
SLA_NO_FRESHNESS_CACHE=1, shared install read-only. Raw output:
probes1.out .. probes5.out in this directory. Inputs lived in the
probe workspace (w/{exp,mac,ws,inline,nest,strx,wstypo,unterm});
the key inputs are reproduced in the line report
(docs/phase4/imp.md) and in the FB sheets.

## Round 1-2 — expand_tuple / macros / workspace / inline / nest

- exp_ok (root `@expand_tuple(2, 3, T)` + generic calls):
  check rc=0, build rc=0.
- exp_bad (`@expand_tuple(3, 2, T)`, min>max): rc=1
  `Macro Expansion Error: failed to expand tuple templates in bad.sla: error.InvalidExpandTuple`.
- exp_each (`@each`/`@join`/`$ORD`/`$P` ParamSet pattern):
  check rc=0, build rc=0.
- mac_get1: `GET_ONE(5)` (expression-output form) check/build
  rc=0; `GET_ONE()` rc=1 `macro GET_ONE expects 2 args or 1 args
  when using expression output, got 0`.
- mac_setpair: `SET_PAIR(a, 1)` rc=1 `macro SET_PAIR expects 3
  args or 2 args when using expression output, got 2`;
  `SET_PAIR(a)` rc=1 (got 1).
- ws_member: from members/app/src, `@import "libx/helper.sai"`
  check rc=0 (package-name resolution works).
- ws_unknown: `@import "nope/helper.sai"` rc=1
  `Import Error: failed to expand @import SLA sources: error.FileNotFound`.
- inline_ok: lib.sla with `helper` + `unused_helper`, main calls
  helper — check/build rc=0; emitted main.sa contains helper,
  unused_helper AND the namespace-alias variants (grep helper = 9
  lines).
- inline_broken: unused_helper body has a type error
  (`return "str"` for i32) — check rc=0 AND build rc=0. This pins
  the decl-only nature of imported bodies on the check path
  (plugin_commands.zig passes parse_function_bodies=false +
  imported_bodies_decl_only=true, include_all_imported_decls=true).
- nest_sla: lib.sla @imports helper.sai and calls the extern
  inside its body; main @imports lib.sla — check/build rc=0.

## Round 4 — FB-045 (string literals)

- str_expand_bad: `"@expand_tuple(x)"` as a string value ->
  rc=1 InvalidExpandTuple (valid program rejected).
- str_expand_rewrite: string containing a well-formed directive
  -> check/build rc=0, but the emitted main.sa's string content
  is the EXPANDED text (` not a directive `); the directive text
  is absent from the artifact. Silent value rewrite.

## Round 5 — FB-046 / FB-047

- ws_typo_root: sa.mod header typo `workspac {` -> check at the
  workspace root rc=1, diagnostic is the bare `error: InvalidFormat`
  with no file/line.
- unterm_macro: `[MACRO]` block without `[END_MACRO]`, called
  from the importer -> check rc=0 (macro registered, body to EOF).

## Rust-oracle cross-checks (same inputs)

- Root @expand_tuple file: oracle rc=0, emitted SA contains
  get2/get3 (this capability is new in phase 4; before, oracle
  had no root source expansion and parsed fn templates as
  malformed — matches its INT2-7-era note).
- Macro probe file (03a shape): oracle rc=0; contracts dump lists
  GET_ONE REGISTERED (arity=2, leading_outputs=1, assigned=0x1,
  from=<path>).
- Fixtures 01/03/04: full-pipeline output byte-identical to the
  phase-3 expected files (shas in docs/phase4/imp.md); 02/07 keep
  only the known D-C23 cleanup-ordering delta (same line
  multiset).
