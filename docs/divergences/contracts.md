# Divergences — INT-2 contract segment (phase 3)

Lane: INT-2 (crates/sla-parser contract family, crates/sla-plugin
wiring, crates/sla-oracle contracts stage). Zig references are to
the venue tree head 6002b386 (local tree
~/workspace/local-sa/sa_plugin_sla; line numbers spot-checked
against both). Evidence: docs/probes/int2/ZIG_PROBES.md (venue
`sa sla check` runs) and the venue oracle runs recorded in
docs/phase3/int2.md.

## INT2-1 (b, 本移植缺陷 — checker lane) Rust checker lacks the Zig name-special call arms; `println`/`str_eq` are NOT contracts

- Zig: `println` and `str_eq` are resolved inside the checker's
  call dispatch by NAME, unconditionally — type_checker.zig
  `isStrEqCall` arm (exactly 2 args, both must be string-like,
  returns boolean) and `isPrintlnCall` arm (checks every argument,
  returns void, ANY arity); name facts in lowering_rules.zig:4196
  / :4172 (`isStrEqCall` / `isPrintlnCall`). The arms sit BEFORE
  user-function resolution, so user code can never shadow them.
  No `.sai` declares them (print.sai declares only
  `sa_print_bytes`); no import is needed.
- Venue proof (sa 0.1.8): a file with NO imports calling
  `println` with 1, 2 and 3 args and `str_eq("a","a")` checks
  rc=0; `str_eq(1, 2)` fails rc=1 TypeMismatch (probe p1/p2).
- Rust: sla-checker's `check_call` has no `println` / `str_eq` /
  `len` / `format` / `hash` / `debug` arms (the neighbouring Zig
  specials at type_checker.zig:4230-4276); such calls fall through
  to the unresolved-call diagnostic. All 5 fixtures die here after
  a complete contract load (docs/phase3/int2.md table).
- Why INT-2 did not "fix" it through registration: D-A2 forbids
  touching checker sources on this lane, and `register_extern_func`
  cannot express these arms — an extern has one fixed arity
  (println takes 1/2/3+ args) and its argument types are bypassed
  (extern `str_eq` would ACCEPT `str_eq(1, 2)`, which Zig rejects).
  Registering them would forge verdicts — the exact failure this
  experiment exists to catch.
- Disposition: registered for the coordinator to sequence to the
  checker lane (same class as INT-1). The plugin test
  `println_remains_blocked_by_missing_checker_builtin_arm` pins
  the current state and must be updated when the arms land.
- **CLOSED by the INT21 lane (2026-11-07, phase 3)**: the whole
  bare-name block at venue :8805-8895 is mirrored in
  `crates/sla-checker/src/check_builtin.rs` (vec / len / str_eq /
  format / hash / debug / println / catch_unwind; 33 venue
  probes, 28 new checker tests, checker 146/146). All 5 fixtures
  now pass the full oracle pipeline; 01/03/04 are byte-identical
  to `fixtures/expected/*.sa`, 02/07 differ only by the D-C6
  cleanup-order pair swap (registered as D-C23 in
  docs/divergences/checker.md). As foreseen above, the plugin
  pin test now fails (2 passed / 1 failed in
  `contracts_pipeline`) and awaits its update on the sla-plugin
  lane / coordinator — its intent has inverted: println is
  unblocked. Full accounting in docs/phase3/int21.md.
- Side note: `str_eq` is only a LATENT second blocker. In build
  mode the @test body containing it is pruned before check; in
  full mode `main`'s `println` is checked first and dies first.

## INT2-2 (b, 注册面不足) `ExternSig` cannot carry `return_fallible`

- Zig `contract_parser.ExternalFunction.return_fallible` (trailing
  `!` on the return type) is stored by `loadContracts` and consumed
  by codegen (codegen.zig:15342) and SAB (sab_codegen.zig:6324).
- Rust `sla_checker::ExternSig` has only `move_params` + `ret_abi`.
  The contract parser and `LoadReport` carry the flag faithfully;
  it is dropped at the `register_extern_func` boundary. Check-time
  behavior is unaffected (the Zig checker never reads the flag);
  the gap matters when the codegen lanes consume extern metadata.
- **INT21 ruling (2026-11-07): NOT blocking the 5 fixtures.**
  Venue grep: the only Zig readers are codegen.zig:2989/19043
  and sab_codegen.zig (reading the extern entry at codegen
  time); no fixture source calls a fallible extern directly
  (their calls are println/str_eq only), and Rust codegen lanes
  do not consume checker `ExternSig` for fallible payloads
  today. Adding the field is one line in sla-checker but its
  construction sites live outside the INT21 jurisdiction
  (sla-plugin/src/contracts.rs `extern_sig_of`, sla-oracle
  src/main.rs shim literal) — a unilateral field addition
  breaks both crates' compilation. Coordinator action: land
  the field + both construction-site updates atomically when
  a fixture/corpus case first calls a fallible extern from
  source and needs the flag plumbed through the checker.

## INT2-3 (b, 注册面不足) No `layout_defines` table in the Rust checker

- Zig `TypeChecker.layout_defines` (type_checker.zig:143) is
  filled by `loadContracts` from `.sal` files (:2115).
- The Rust `Checker` has no such table. Fixture 01's closure alone
  carries 323 defines; they are parsed and carried in the
  `LoadReport` / oracle contracts dump — visible, counted, and
  explicitly unregistered, not silently dropped.
- **INT21 ruling (2026-11-07): NOT blocking any stage.** Venue
  full-tree grep: in Zig, `layout_defines` is written by
  `loadContracts` (type_checker.zig:6064) and deinitialized —
  it has NO reader anywhere (no `.get` in checker, lowering or
  either codegen). The table is inert registration metadata;
  its Rust absence changes no verdict. No table will be added
  until Zig grows a first reader (adding an unread registry
  would be motion without effect); the `LoadReport` carriage
  keeps the 323 defines auditable meanwhile.

## INT2-4 (a, 覆盖缺口 — 分阶段剩余) Imported macros are reported, not registered

- Zig's contract recursion also runs
  `loadImportedMacrosFromExpandedSource` per file
  (plugin_import_expand.zig:1358/1390), filling
  `tc.imported_macros` from SA-text `[MACRO]` blocks.
- This slice registers externs only (per the task's slice
  definition). Macro names encountered are collected per file
  into the report (fixture 01: 13 files) so the omission is
  auditable; the plugin layer additionally exposes
  `expand_sla_imports_unwired()` as a loud refusal for the rest of
  plugin_import_expand.zig (`.sla` body inlining, module tables).

## INT2-5 (a, 覆盖缺口) Workspace-package import resolution not ported

- Zig `resolveImportFile` tries `resolveWorkspacePackageImport`
  (workspace.zig manifest machinery) before relative resolution.
- Not ported: workspace-shaped paths fall through to relative
  resolution and fail with a loud FileNotFound. No fixture or
  std import uses this route.

## INT2-6 (a, 有意分歧 — 承 D-C3) Hard-coded / home fallback roots omitted

- `resolveSaStdImport` / `resolveSlaStdImport` end their chains
  with `~/projects/...` guesses and the literal root
  `/home/vscode/projects/sci/sa_std` (plugin_imports.zig).
- Per the standing D-C3 registration these are not ported; the
  ported chain is ancestor roots -> `SA_STD_DIR` / `SLA_STD_DIR` /
  `SCI_ROOT` env roots -> plugin home -> relative paths.

## INT2-7 (a, 覆盖缺口) `@expand_tuple` sources in the contract closure fail loudly

- Zig runs `source_expand.expandForModulePath` over every resolved
  file before scanning/parsing it.
- The port implements the identity fast path only; a closure
  source containing `@expand_tuple` returns
  `ContractLoadError::UnsupportedExpandTuple` instead of scanning
  an unexpanded text and registering a wrong contract set. None of
  the 5 fixtures' closures (30 files for fixture 01) uses it.

## INT2-8 (a, 有意镜像) contract_parser.zig quirks mirrored verbatim

- Parameter lists split on every `,` (a generic type containing a
  comma yields a segment without `:` -> SaiParseError; unit test
  `comma_inside_param_type_splits_like_zig`).
- Parameter `:` split reads only the first two segments.
- Return type = trimmed rest of line after `->`; trailing `!`
  marks fallible.
- `#def` values that fail i64 parsing become 0 (Zig `catch 0`) —
  noted as a silent-swallow smell of the D-C2 family, but it only
  feeds layout metadata; mirrored and recorded here rather than
  spending a feedback slot.
- Duplicate extern names across imports: later registration
  overwrites earlier (Zig `extern_funcs.put`), in deterministic
  load order (declaration order, scan-before-load per file,
  per-import lists sorted by output_path).

## Feedback slots

FB-021..FB-024: **none issued**. Every finding on this lane is a
port-side gap (INT2-1..INT2-7) or a mirrored quirk (INT2-8); no new
Zig-side defect candidate met the bar (the println/str_eq
shadowing behavior is coherent across checker and both codegens —
a builtin surface, not a defect). Slots returned unused.
