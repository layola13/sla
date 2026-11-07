# Phase 4 — import & expansion surface (IMP line)

Baseline: d811673c (phase-3 consolidation). Zig reference: box
sa_plugin_sla HEAD 2cc12ea8 (newer than the local read-only tree;
all Zig line references use the box head). Venue: Colab box,
HOME=/content/plhome-rust4, heavy work under
flock /content/rust4/heavy.lock. No contention with Zig gate
lines was encountered during this line's runs (every lock
acquisition was immediate); per the 2026-11-07 14:44 priority
ruling this line would have taken precedence had there been any.

## Item status

1. **Import macro system — WIRED + cross-checked.**
   `sla-parser/src/imported_macros.rs` (562 lines) mirrors
   plugin_imported_macros.zig end to end (record, param/output
   analysis, EXPAND-mask + callee propagation, loader over
   expanded source and over files). Registries are Vecs in file
   order (deterministic; later registration replaces in place =
   Zig `put`). Contract loading collects the registry in Zig
   order; `sla-plugin/src/contracts.rs::load_imported_contracts_into`
   registers it into the checker; the oracle registers it for its
   dump. Venue: arity diagnostics match the box byte for byte
   (`macro SET_PAIR expects 3 args or 2 args when using
   expression output, got N`); expression-output gating
   (leading_outputs==1) matches `importedMacroUsesExpressionOutput`.
2. **Workspace packages — WIRED + cross-checked.**
   `sla-parser/src/workspace.rs` (658) mirrors workspace.zig
   (manifest parse incl. silent-ignore quirks, members /
   default_member, DuplicateEntry / UnknownPackage /
   MissingDefaultMember, member listing, selection, nearest /
   ancestor roots, resolve / selected_source_path).
   `imports.rs::resolve_workspace_package_import` is wired into
   resolve_import_file in Zig's order: sla_std -> sa_std ->
   plugin -> **workspace** -> relative. Venue: member import
   rc=0; unknown package degrades to relative and fails loudly
   (FileNotFound), exactly like Zig.
3. **@expand_tuple — WIRED + cross-checked.**
   `sla-parser/src/source_expand.rs` (522) mirrors
   source_expand.zig (expand + expand_for_module_path, @each /
   @join, $TYPE_PARAMS/$TYPES/$N/$I/$ORD/$<prefix>/$T, ordinal
   cap 16, InvalidExpandTuple on min>max and malformed bodies).
   ContractLoad's UnsupportedExpandTuple is gone — imported
   sources are really expanded; the oracle expands the ROOT
   source before parsing (it previously had no root expansion).
   Venue: root file rc=0 both sides; min>max diagnostic matches;
   ParamSet @each/@join shape rc=0.
4. **.sla function-body inline expansion (expandSlaImports) —
   WIRED for the check-path combination, loudly unwired for the
   rest.** `sla-plugin/src/sla_expand.rs` (611) mirrors
   plugin_import_expand.zig's module table + collect +
   appendModuleDeclsSelective for the combination the real
   `check` command uses (dir-mode reject + include_all_imported_
   decls=true; decl-only bodies): imported decl-only function
   conversion, impl/overload filtering, test fns dropped, macros/
   consts kept, per-module non-.sla imports NOT retained, root
   non-.sla imports appended unconditionally + contract imports
   collected. The default selective mode, prune/lazy/registry
   combinations return `SelectiveUnwired` (IMP-1) — loudly, at
   the API face. Venue pin: an imported body with a deliberate
   type error passes check AND build on the real plugin; the
   Rust pipeline reproduces that through the decl-only path
   (integration test). The nested .sla->.sai case reproduces via
   raw expansion + the contract closure.

## Files

New: crates/sla-parser/src/{source_expand.rs, imported_macros.rs,
workspace.rs, workspace/tests.rs},
crates/sla-plugin/src/{sla_expand.rs, sla_expand/tests.rs},
crates/sla-plugin/tests/import_expand_pipeline.rs,
docs/divergences/import-expand.md, docs/probes/imp/*,
docs/feedback/FB-045..048, docs/phase4/imp.md.
Changed: crates/sla-parser/src/{lib.rs, imports.rs,
contract_load.rs}, crates/sla-plugin/src/{lib.rs, contracts.rs},
crates/sla-oracle/src/main.rs,
docs/divergences/contracts.md (closure notes appended).

## Tests

- sla-parser: lib 35 + fixture-integration 12, all green.
- sla-plugin: lib 25 + contracts_pipeline 3 +
  import_expand_pipeline 4, all green.
- Workspace total: **324 passed / 0 failed** (baseline 294;
  +30). Full-pipeline fixtures: 01/03/04 byte-identical to
  phase-3 expected (sha256 0d6375ac… / 5a92ee82… / 60f81478…);
  02/07 differ only in the known D-C23 cleanup line ORDER (line
  multisets equal) — no regression.
- check_file_lines.sh: all 136 .rs files <= 700 lines
  (sla-oracle/src/main.rs exactly 700).

## Divergence / feedback index

IMP-1..IMP-8 in docs/divergences/import-expand.md (IMP-7 records
the federation-by-construction shape per the 2026-11-07 14:37
ruling; IMP-8 = FB-045). Feedback candidates FB-045..FB-048 in
docs/feedback/. The sla_std-rule gap (wrapper generator not
mirrored in this repo; direct .sai imports remain compatible,
sla_std stays the mainstream face) is registered at the end of
docs/divergences/import-expand.md.
