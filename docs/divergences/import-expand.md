# Divergences — phase-4 import & expansion lane (IMP)

Lane: IMP (crates/sla-parser source_expand / imported_macros /
workspace / imports / contract_load, crates/sla-plugin contracts /
sla_expand, crates/sla-oracle contracts + root-expansion stages).
Zig references are to the BOX head 2cc12ea8bb38757a611a0d674bc2002e1eda7931
(/content/sa_all/sa_plugin_sla) — the box tree is NEWER than the
local read-only tree (6002b386-era): plugin_imported_macros.zig
420 vs 288 lines, source_expand.zig 468 vs 450, plugin_import_expand
1512 vs 1447, type_checker >10000 vs 6098. All line citations below
are box-head. Evidence: docs/probes/imp/PROBES.md (venue `sa sla`
runs, shared installed plugin, read-only use).

## IMP-1 (a, 覆盖缺口) Selective `.sla` expansion remains loudly unwired

- Zig's default expansion mode is reachability-selective
  (plugin_reachability.zig 2,762 lines + the reachability arms of
  plugin_import_expand.zig + plugin_module_table.zig reparse /
  materialization machinery).
- Phase 4 wires ONLY the `include_all_imported_decls` combination
  (the real `sla check` command's mode, plugin_commands.zig at the
  box head) in `sla_plugin::sla_expand`. The default combination
  and every combination depending on reachable sets
  (prune_for_test_codegen, lazy_transitive_sla_imports,
  load_reachable_imported_bodies_from_registry) return
  `SlaExpandError::SelectiveUnwired`; the old plugin entry
  `expand_sla_imports_unwired()` remains as the loud default-mode
  refusal. No silent partial expansion anywhere.
- Update (module-expand continuation, 2026-11-08): the wired
  include-all combination is now actually invoked by the CLI
  Check front-end — previously it ran only in the plugin /
  oracle paths and the CLI's TestCodegen kind, so `sla check`
  itself still saw unexpanded programs — together with the
  IMP-4 signature half (see IMP-4). The selective default
  remains unwired exactly as above, and the Compile kind still
  runs no expansion; both stay registered residuals here.

## IMP-2 (b, 注册面不足) `ImportedMacroInfo` cannot carry `assigned_output_mask`

- Zig `ImportedMacro` (type_checker.zig:28-37 at box head) carries
  `assigned_output_mask`; the loader computes it
  (markAssignedOutputMacroParams). The Rust record carries it
  faithfully; `sla_checker::ImportedMacroInfo` has no such field,
  so it is dropped at `register_imported_macro`, exactly the
  INT2-2 shape (ExternSig.return_fallible). The Rust checker does
  not consume the mask today; coordinator action identical to
  INT2-2: add the field + construction sites atomically when a
  consumer lands.

## IMP-3 (b, 本移植缺陷 — checker lane) Rust imported-macro call arm lacks place-overlap validation

- Box-head Zig (type_checker.zig:10043+) validates pairwise
  place overlaps for imported macro calls, with borrowed slots
  taken from `borrowed_arg_mask | address_slot_arg_mask`.
- The Rust checker's `check_imported_macro_call` (phase-2 vintage)
  checks arguments and arity only. The masks ARE now registered
  (this lane), so the data the validation needs exists; the
  validation itself is checker-lane work, registered here.

## IMP-4 (a, 覆盖缺口 → 签名半边已接，module-expand 续跑线 2026-11-08) Namespaced alias registration

- Zig `registerImportedFunctionAliasesFromResolvedImports`
  registers imported functions under bare AND `namespace__name`
  symbols for the checker (check path runs it right after contract
  loading), plus impl methods under mangled symbols only, with
  overload-declaration methods outside the surface.
- Signature half NOW WIRED: the expansion returns the root
  groups' `.sla` modules (`ExpandedProgram::root_sla_modules`),
  and the CLI front-end registers their exported signatures in
  Zig's order — after contract loading, before check_program —
  via sla-cli's `pipeline::expand_wire`, 1:1 with the Zig
  surface above. It runs on every kind that expands (Check
  decl-only, TestCodegen bodies-kept).
- Residual 1 (alias metadata table): Zig's
  `registerFunctionAliasWithMetadata` also fills the checker's
  function-alias metadata tables. Not ported — no Rust consumer
  reads them; port atomically when a consumer lands (the IMP-2
  coordinator shape).
- Residual 2 (Compile kind): the Compile front-end runs no
  expansion (IMP-1), so it registers nothing either; Zig's
  standalone entry, which re-resolves root imports without an
  expansion, is not mirrored. Closing either residual must not
  wire decl-only data into the Compile path — that would fake
  check-green against missing bodies (see IMP-1).

## IMP-5 (b, 近似) Module type prescan is a textual approximation

- Zig parses modules with the parser's own filesystem prescan
  (`prescanSlaImportTypes`, parser.zig; D-P11). The Rust module
  table seeds `Parser::seed_known_type_names` from a textual scan
  of the transitive `.sla` imports' `struct`/`enum` lines.
  Decl-only mode (the wired check path) is insensitive to the
  difference in the probed cases; raw-body mode may miss names
  whose declarations the textual scan does not recognise
  (generated/aliased forms). Registered, not hidden.

## IMP-6 (a, 有意省略 — 性能面) Disk caches not ported

- Zig memoizes three pure computations on disk under
  `.sla-cache/`: expand/ (source_expand), macros/*.idx
  (plugin_imported_macros), imports/*.lst (import-list scan).
  All three keys include a compiler stamp (cache_version) at the
  box head. None is ported: each memoizes a deterministic
  function, so outputs are identical; only speed differs. The
  Wyhash key construction also cannot be reproduced without a new
  dependency (crate graph frozen). If a future lane needs cache
  INTEROPERABILITY with the Zig plugin (shared .sla-cache), that
  is a separate ruling — today the Rust side never reads or writes
  `.sla-cache`, so no cross-contamination is possible.

## IMP-7 (有意为之 — 设计裁定) Registration is declaration-generated and merge-ordered, per the federation ruling

- Design constraint (user ruling 2026-11-07 14:37, originating
  2026-11-06 12:15): surface registration in this repo is designed
  "generated from declarations, federatable by package". Where
  this lane touches registration it follows that shape: extern
  and imported-macro records are GENERATED from each imported
  file's own declarations at load time (nothing is baked into a
  central table by this lane), merged in deterministic load order
  (nested-before-importer, declaration order), and each record
  retains provenance (`import_path` = the file's output path;
  oracle dump prints it per record). Duplicate names resolve
  later-wins in place — Zig's `put` semantics — which is the
  merge rule a per-package manifest merge would need.
- NOT touched by this lane: the SAB surface registry (SAB lane's
  baked table) and any per-package surface-manifest format. No
  manifest format was invented here; the workspace manifest
  (`sa.mod`) carries no surface list in Zig and none was added
  (no improvements).

## IMP-8 (c, Zig 侧缺陷候选) `source_expand` scans raw bytes, blind to strings/comments

- See FB-045 (docs/feedback/FB-045-expand-tuple-inside-string.md):
  venue-probed in two forms — a valid string literal containing
  `@expand_tuple(x)` is REJECTED (InvalidExpandTuple), and a
  well-formed directive inside a string literal is silently
  EXPANDED, rewriting the string's value in the emitted program.
  Rust mirrors the raw scan verbatim (no improvements); the
  divergence is the feedback sheet, not a behavior difference.

## Feedback sheets issued by this lane

- FB-045 — source_expand string/comment-blind scan (above, IMP-8).
- FB-046 — workspace manifest: unknown top-level lines silently
  ignored in `parseManifestWithFile`; malformed manifests surface
  at the command layer as a bare, unlocated `error: InvalidFormat`
  (venue probe: typo'd `workspac {` header, rc=1, no file/line).
- FB-047 — unterminated `[MACRO]` block silently registers with
  its body running to EOF (venue probe: rc=0, macro callable);
  any file content after the header is swallowed into the body
  with no diagnostic.
- FB-048 — macro index cache (`tryLoadImportedMacrosFromCache`)
  skips individual records whose numeric fields fail to parse
  (`catch continue`) while registering the rest from the same
  cache file — partial registration from a corrupt cache; only
  the missing-assigned-field case forces a full rescan.
  Source-cited (the Wyhash cache path cannot be crafted from
  outside without the digest), observation level.

## sla_std import-form rule — compliance note (user 2026-11-07 14:39, corrected 14:40)

- The rule is mainstream-vs-legacy, NOT compatibility: client code
  should write `@import "sla_std/xxxx"` (generated thin wrappers);
  `@import "sa_std/xxx.sai"` direct form MUST keep resolving
  (no error, no warning). This lane's resolution keeps both:
  `resolve_import_file` tries sla_std BEFORE sa_std (Zig's own
  order, mirrored since phase 3), and direct `.sai` resolution is
  unchanged — no new diagnostic was added anywhere for the direct
  form.
- Gap registration (generator): the sai->sla_std wrap generator
  (Zig-side `sai-wrap-pilot/gen_wrap.py` + `sla_std/wrap/`, one
  file per .sai, generated files not hand-edited, .sai changes
  regenerate in the same batch) has NO counterpart in this repo:
  `grep -rn "gen_wrap|sai-wrap|sai_wrap|sla_std/wrap"` over the
  whole phase-4 tree returns zero hits. Nothing in
  sla-plugin-rust generates, verifies, or consumes generated
  wrappers as a distinct surface — `sla_std/...` imports resolve
  to whatever files exist under the sla_std roots, generated or
  not. Closing that gap is generator-lane work; per instructions
  this lane registers it and does NOT migrate stock corpus.
- This lane's new probes/tests use relative and package imports
  for implementation-layer contract files (`.sai`/`.sa` fixtures
  are the implementation layer itself); no new client-form
  `sa_std/*.sai` import was introduced.
