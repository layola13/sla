# R-2 Replay — RESULT

Port of the true R-2 fix (single commit `49c42a6` on base `cc52e91`, tree
`~/workspace/rust-r2-fix/`) onto current head `3604444` (which contains the
R-1 fix), fully re-verified on this machine. Composition morphology #1 held;
no second morphology was needed and no Jev semantic fork was triggered (the
one candidate fork — the macro-registration scope — was resolved by direct
evidence on the head itself, §2.4b).

**Deliverable commit (composition): `22db492c3fc20aea645822aac22f150a88cf322a`**
on top of `36044442041605a177a968bd6421b324ca59e192`. 25 files changed,
3872 insertions(+), 166 deletions(-). (An earlier draft of this commit,
`b56c298f18e353116391f5ea2fe9aaadefe774bb`, additionally swept in two
`.sla-cache` freshness artifacts produced by the pipeline's own unit tests;
it was soft-reset before delivery and the artifacts removed. `b56c298`
remains a valid local object but is **not** part of the deliverable chain.)
This RESULT.md is committed as a second commit on top, together with the
rename of the R-1 report it supersedes at this path (`RESULT.md` →
`RESULT-R1.md`, content unchanged); the second commit's sha is reported
with the delivery.

No push was made and no remote branch was created; landing is the main
session's action (PAT route).

---

## 1. What was ported

R-2's root causes (per `~/workspace/rust-r2-fix/RESULT.md`), all three
addressed by the port:

1. **Checker std leaves never ported** (UNDEF-STD): the name-keyed
   associated-call cascade and the receiver-shape method cascade for std
   containers were missing from the Rust checker.
2. **CLI never wired `.sla` import body expansion into the TestCodegen
   form** (UNDEF-MODULE): `sla test` bodies calling imported module
   functions saw only a decl surface.
3. **Imported SA macros never registered in the CLI checker on the
   TestCodegen form**.

Plus R-2's four text-codegen gap fixes (codegen-text
`call.rs`/`expr.rs`/`program.rs`/`stmt.rs` + new `family_macros.rs` and
`surface.rs`), the `sla_expand` work in sla-plugin, and its divergence-doc
registrations.

Changed set vs head `3604444`: 16 modified + 9 new paths.

- **New**: `crates/sla-checker/src/check_std_ctor.rs`,
  `check_std_leaf.rs` (666 lines), `check_std_leaf2.rs`,
  `check_std_leaf3.rs`, `tests_std_leaf.rs`,
  `crates/sla-codegen-text/src/family_macros.rs`, `surface.rs`,
  `crates/sla-cli/src/pipeline/tests.rs`, and — replay-only —
  `crates/sla-checker/src/check_assoc_helpers.rs` (§3.2).
- **Modified**: `crates/sla-checker/src/{types_util.rs, lib.rs,
  check_call.rs, check_method.rs}`,
  `crates/sla-cli/{Cargo.toml, src/pipeline.rs}`,
  `crates/sla-codegen-text/src/{call.rs, expr.rs, lib.rs, program.rs,
  stmt.rs}`, `crates/sla-plugin/src/sla_expand.rs` (679 lines) +
  `sla_expand/tests.rs`, `docs/divergences/{checker.md, cli-commands.md,
  lowering-text.md}`.

## 2. Overlap composition register (5 files)

R-1 and R-2 are two independent ports of the *same* Zig std-callable-surface
cascade, written against different bases. Governing ruling applied
throughout: **current head (R-1) keeps first refusal on every shared name;
R-2's cascade is consulted only on an R-1 miss.** No shared name is
double-counted and neither side overwrites the other.

### 2.1 `crates/sla-checker/src/check_call.rs` — associated-call wiring

Both lines insert into `check_associated_call` at the same anchor (after the
`ptr` leaves, before the inherent `{Target}_{name}` lookup).

- R-1's `check_std_assoc_leaf` (check_leaves.rs) stays first, unchanged.
- R-2's `check_std_associated_leaf` (check_std_ctor.rs) is inserted
  immediately after it.
- The doc comment at the dispatch was composed to state this order
  explicitly ("…check_leaves.rs, R-1 port; then the R-2 constructor leaves,
  check_std_ctor.rs, consulted only on an R-1 miss…"), and the insertion
  site carries its own comment recording the same.

### 2.2 `crates/sla-checker/src/check_method.rs` — receiver wiring

No positional conflict existed. R-1 wired its receiver cascade
(`check_receiver_leaf_call`, check_receiver_leaves.rs) from `check_call.rs`
*before* `check_receiver_call` is consulted; R-1's change to this file was
doc-comment only. R-2's hunk appends inside `check_receiver_call` itself.

- Composed as new step 5 at the end of `check_receiver_call` (before the
  final `None`): `check_std_receiver_leaf` then `check_std_receiver_leaf2`,
  with a comment noting inherent/trait/dyn/`using` keep precedence and that
  the R-1 receiver cascade has already had first refusal on every shared
  name, so a leaf here can only resolve a call every earlier stage missed.

### 2.3 `crates/sla-checker/src/lib.rs` — module declarations

Union of both sides: R-2's `mod check_std_ctor; mod check_std_leaf;
mod check_std_leaf2; mod check_std_leaf3;` after `mod check_stmt;`, and
`#[cfg(test)] mod tests_std_leaf;` after the R-1 test module. No conflicts.

### 2.4 `crates/sla-cli/src/pipeline.rs` — body expansion (taken whole)

R-2's patch applied, then two adjudications:

- **(a) `.sla` body expansion (root cause 2): taken whole.** A
  TestCodegen-only block runs `sla_plugin::sla_expand::expand_sla_imports`
  (include-all, bodies kept) after parse and *before* the CLI-D5 prunes, so
  the reachability prune sees the module functions tests actually reach.
  The expanded module list joins the freshness closure
  (`closure.extend(expanded_modules…)`). Check/Compile stage order is
  byte-identical to before. The module doc comment was updated to match
  (expansion is no longer listed among the unported surfaces for the
  TestCodegen kind; it remains unported for Check/Compile, as R-2
  deliberately left it).
- **(b) Macro registration (root cause 3): R-2's duplicate loop REMOVED.**
  R-2 added a TestCodegen-scoped registration loop; R-1 had already added
  an unconditional all-kinds loop with an identical body on this head.
  Keeping both would register every imported macro twice (a no-op
  re-insert of identical records). The composed tree keeps exactly R-1's
  loop; its comment was rewritten to record that R-1's all-kind
  registration is the superset and that the Check/Compile-kind gap R-2
  deliberately left open is already closed on this head. This is the one
  place where the two lines' *scopes* differed; it was resolved by
  inspection of the head (the loop is unconditionally present), not by
  preference — no Jev escalation was needed because there was no semantic
  disagreement about any call form, only a scope subsumption.

### 2.5 `docs/divergences/checker.md` — divergence registration

R-2's appended section was coherent at EOF but two of its claims are
superseded by R-1 on this head (its wiring description predates R-1's
cascade, and it lists `future::`/`poll::` surfaces as unported, which R-1's
preassoc delegation covers). Rather than rewriting R-2's text, the replay
appended an annotation recording the R-1-first composition order and the
coverage correction (`task::`/`executor::` remain unported). A matching
annotation was appended to `docs/divergences/cli-commands.md` after its
Check/Compile-gap paragraph, recording that on head `3604444` R-1 already
closed that gap for all kinds and that R-2's duplicate TestCodegen loop was
removed in the replay. No stale claim remains unannotated.

## 3. Composition defects found by the first compile — replay-only fixes

Neither parent tree is broken alone; both defects are artifacts of
composing two independent ports of the same cascade. Both are registered
here as changes *beyond* `49c42a6`.

### 3.1 Duplicate inherent method names (E0592 ×3, E0034 ×25)

The two cascades define same-named `Checker` helper methods with
**different semantics** (verified by body diff — they are not
interchangeable, so neither could simply be deleted):

| name | R-1 copy (kept the name) | R-2 copy (renamed) |
|---|---|---|
| `consume_insert_arg` | check_receiver_leaves.rs:274 — eligibility gating + `concretize_receiver` machinery | check_std_leaf.rs — stricter Zig insert transfer fact (Pointer exemption, root-identifier self-check, Consumed→UseAfterMove). Renamed → **`consume_std_insert_arg`** (def + 6 call sites in check_std_leaf3.rs) |
| `check_closure_arg` | check_receiver_leaves.rs:148 — closure-*literal* checking with param-context inference, 3-arg signature | check_std_leaf.rs:192 — plain check + Closure destructuring, 2-tuple return. Renamed → **`check_std_closure_arg`** (def + 4 calls in check_std_leaf.rs + 3 calls in check_std_leaf2.rs) |
| `consume_constructor_arg` | check_leaves.rs:112, `pub(crate)` — returns Ok on unknown names via `lookup_state` | check_std_ctor.rs:151, `pub` — errors `UndefinedVariable` on unknown names, keys Copy on `type_is_copy`. Renamed → **`consume_std_constructor_arg`** (def + 1 call site, check_std_ctor.rs:253) |

R-1's versions keep the original names and first-refusal semantics
everywhere — consistent with the §2 ruling. After the renames,
`cargo test -p sla-checker` compiles clean.

### 3.2 `check_call.rs` line cap — verbatim split

`check_call.rs` was 699 lines at head; the composed wiring pushed it to
713 > 700. Resolved without a second morphology and without trimming any
semantics: the `sum` special arm was extracted **verbatim** into the new
file `crates/sla-checker/src/check_assoc_helpers.rs` as
`check_sum_call(_checker, target_ty)` (the arm never used `self`; the
parameter is an underscore placeholder so the call site stays one line),
and `check_call.rs` now calls it. A follow-on E0432 in the new file
(`crate::types` does not exist; `Type` lives in `sla_ast::types`) was fixed
in its use-line.

`./check_file_lines.sh` on the final tree: **208 .rs files checked, all
≤ 700** (largest: check_receiver_leaves.rs 694, check_std_leaf.rs 666).

## 4. Verification ① — cargo workspace

`cargo test --workspace` on the replay tree (cargo 1.99.1):
**539 passed / 0 failed.**

Baselines, same caliber: head `3604444` = 526/0; true body `49c42a6` =
513/0. The replay is ≥ both — only-increase satisfied. The +13 over head is
exactly R-2's own new tests (tests_std_leaf 9 + sla_expand 2 + pipeline 2);
no test was lost or weakened in composition.

## 5. Verification ② — corpus three-way rerun

### 5.1 Harness and caliber

Harness (`rerun/r2run.sh`, outputs in `rerun/run_{stock,r2fix,replay}.tsv`
with per-run logs alongside): for each of the **567 non-negative `.sla`
files directly in `corpus/tests/`** (718 `.sla` total there, minus 151
`test_error_*`/`neg_*` negatives), run `sla test <file>` with
`--test-backend sa` and `--test-backend sab` — **1134 runs per tree**,
120 s timeout per run, recording rc, `[PASS]` count, and first error line.
Denominator anchor (corpus/CENSUS.md): @test total = 2513.

Binaries: stock = fresh local build of `cc52e91`; r2fix =
`~/workspace/rust-r2-fix/target/debug/sla` (`49c42a6`, built for this
verification); replay = the replay tree's build of commit `22db492`.

### 5.2 ⚠️ Venue calibration — read this before quoting any absolute number

**The local venue cannot reproduce the box's absolute PASS numbers, for
reasons that affect all three binaries identically:**

- Local stock whole-file PASS = **0** (box census baseline: 25 runs).
- The local host `sa` is `~/workspace/local-sa/sci/zig-out/bin/sa` (0.1.8
  stock-era build; local sci head `1ec669ad`, `libsa_std.a` dated
  2026-11-05) — not any landed head.
- The local sa_plugin_sla tree's `sla_std/` contains only `cmp.sla` and
  `std_surface.sla_meta` — **there is no `wrap/` directory locally**, so
  every `sla_std/wrap/*` import fails resolution on all three binaries
  equally (163 FileNotFound runs per fixed tree).
- Delegated `sa test` additionally needs `zig` on PATH
  (`~/workspace/local-sa/zig17`); without it, links fail with
  ExternalCompiler FileNotFound.

The rerun is therefore valid **only as a same-venue three-way relative
comparison** (identical environment for all three trees). Absolute PASS
counts must be quoted from the box RESULTs (census baseline and the R-2
RESULT), never from this rerun. All conclusions below are relative.

### 5.3 Three-way results (local caliber, 1134 runs each)

First-error bucket counts:

| bucket | stock `cc52e91` | r2fix `49c42a6` | replay `22db492` |
|---|---|---|---|
| **Undefined call** | **580** | **94** | **16** |
| TypeMismatch | 68 | 214 | 274 |
| SAB Error | 62 | 193 | 206 |
| FileNotFound (venue: no `wrap/`) | 162 | 163 | 163 |
| UnsupportedFeature | 49 | 138 | 151 |
| SyntaxError | 116 | 132 | 132 |
| UndefinedVariable | 20 | 74 | 66 |
| ExternalCompiler | 12 | 29 | 29 |
| InvalidTag | 17 | 19 | 19 |
| ForbiddenSyntax | — | 17 | 17 |
| UseAfterMove | — | — | 12 |
| rc=101 (panic) runs | 6 | 14 | 14 |

Whole-file PASS and @test PASS sums are 0 for all three trees *in this
venue only* (§5.2); the box-caliber anchors remain: census baseline
whole-file PASS 25 runs (sa 13 + sab 12), @test PASS sa 17 / sab 19;
R-2 RESULT final whole-file PASS 41 (sa 29 + sab 12), @test PASS sa 57 /
sab 30.

Reading of the table — the R-2 signature is unmistakable and the replay
*exceeds* the true body in this caliber:

- Of the 580 runs whose first error on stock is `Undefined call`, the
  residual is **94 on r2fix and 16 on replay**. (The R-2 RESULT's box
  subset figure was 274→2 on its 274-run R-2 subset; different subset and
  venue, same direction and character.)
- Failure mass moves *downstream* exactly as the R-2 RESULT describes:
  runs that used to die in the checker now reach codegen and fail there
  (TypeMismatch 68→214→274, SAB Error 62→193→206, UnsupportedFeature
  49→138→151). These are the R-4/R-5 residual families, not regressions.

### 5.4 Replay vs r2fix, run by run (the acceptance test)

- **1046 / 1134 runs are identical** (same rc and same first-error bucket).
- All 88 differing runs are accounted for:
  - **78 advancement**: replay's first error is strictly downstream of
    r2fix's — 60× Undefined call → TypeMismatch, 9× Undefined call →
    UnsupportedFeature, 9× Undefined call → SAB Error. Cause: R-1's
    cascade (absent from the true body's base) resolves the call first,
    with its stricter `consume_std_insert_arg` transfer fact then
    surfacing the downstream verdict.
  - **8 lateral**: UndefinedVariable → UnsupportedFeature / SAB Error
    (Arc-family files, e.g. `test_unit_arc_drop_churn`): the composed
    cascades consume the name differently; both trees fail, replay fails
    in a later stage.
  - **2 venue noise**: one ExternalCompiler→FileNotFound and one reverse
    flip (toolchain PATH sensitivity, §5.2).
- **Zero runs regress**: no run that r2fix passes fails on replay (locally
  vacuous — both pass 0 — but:)
- **Zero composition-regression candidates**: among the 580
  stock-Undefined-call runs there is **no** run where r2fix advanced past
  `Undefined call` but replay fell back to it.
- **Panic family unchanged**: rc=101 run sets are identical (14 = 14,
  intersection 14) — the text-codegen `block.rs:191` subtract-with-overflow
  panic family, pre-existing in the true body (present even where stock
  also panics), untouched by the composition.

**Acceptance verdict: the replay is not worse than `49c42a6` in the same
caliber — it is strictly better on the R-2 metric (Undefined-call residual
16 vs 94) thanks to R-1's cascade underneath — and it is dramatically
better than stock (580 → 16).** The brief's second acceptance clause
("markedly better than `3604444` alone") is satisfied structurally: the
replay *is* `3604444` + R-2, and the composed tree carries both cascades;
§5.4 shows their interaction is purely additive (advancement only, no
interference).

### 5.5 Replay residual Undefined-call runs (16 runs / 8 files)

`test_min_reg3_vec_extend`, `test_min_reg3_vec_f64`,
`test_min_reg3_vec_fromvec`, `test_min_reg3_vec_i32`,
`test_min_reg3_vec_u16` (Vec generic extend/from families),
`test_unit_go_chan_view`, `test_unit_go_forget_retire`,
`test_unit_sabgaps_macro_callee_deps` — each on both backends. These are
genuine residuals for the R-4/R-5 queue, recorded here so the next census
can diff against them.

## 6. Verification ③ — minims, one per R-2 root cause

Environment: SA_PLUGIN_DEV=1, SLA_NO_FRESHNESS_CACHE=1, llvm14 shim
LD_LIBRARY_PATH, PATH with zig17 + host sa. Minim sources preserved in
`rerun/minims/`.

### 6.1 Root cause 1 — std leaf cascade (`minim_std.sla`)

Ordinary fn body uses `Vec::new`/`push`/`len` + `HashMap::new`/`insert`;
an @test calls it.

- `cc52e91` stock `sla check`: **rc=1** (`Undefined call: new`).
- Replay `sla check`: **rc=0**. r2fix check: rc=0.

Honesty note: the brief asked for a stock-`3604444`-red control, but R-1
already closed root cause 1 at check level on `3604444`, so no such red
exists; `cc52e91` is the honest red control and is used as such.

Test-form of the same minim: replay and r2fix fail **identically** at the
host stage with `UnknownRegister: call @sa_mem_free(^__sla_map_insert_free_tmp_5)`
— a text-codegen residual in the map-insert free-temp path, present in the
true body `49c42a6` itself, therefore **not** a composition defect.
Registered here as an observed R-4-family residual (not previously in
CLAIMS).

### 6.2 Root causes 2+3 — CLI `.sla` body expansion + imported-macro registration (`minim_module.sla` + `lib.sla`)

`minim_module.sla` does `@import "lib.sla"`; its test body calls the
imported `lib_add` and uses the Vec surface.

- Stock test-sa: fails with `Undefined call: new` (test form sees neither
  the module body nor the std surface).
- **Replay test-sa: PASSES** — `[PASS] imported module fn in test body`,
  `test result: ok. 1 passed`.

On `3604444` the Check-form registration gap (root cause 3) is already
closed by R-1, so no stock-`3604444`-red minim exists for it; the
TestCodegen minim above is the substitute, as pre-authorized by the brief's
fall-back. A corpus-carried imported-macro minim
(`test_smoke1_identity.sla`, uses STR_PTR from `sa_std/string.sa`) could
not be executed locally: it imports `sla_std/wrap/identity.sla` and the
local tree has no `wrap/` (§5.2) — both binaries fail import resolution
identically. Recorded as venue-blocked, not as a result.

## 7. Discipline checks

- **File cap**: `./check_file_lines.sh` — 208 .rs files, all ≤ 700 (§3.2).
- **No std HashMap iteration order in emit paths**: the port introduces no
  new emit-path consumers of HashMap/HashSet order; the verbatim files are
  the true body's, and the composed wiring adds only ordered cascade
  lookups (name-keyed match arms, Vec-ordered module lists). The freshness
  closure extension iterates `expanded_modules`, an ordered Vec produced
  by the expansion's `ordered_modules`.
- **Morphology stop-loss**: not triggered — composition morphology #1
  (head-first-refusal + verbatim split for the line cap) closed every
  signature.
- **Jev checkpoint**: not triggered (§2.4b — scope subsumption resolved by
  evidence on the head, no semantic disagreement on any call form).

## 8. Git object existence declarations

Every sha cited in this RESULT was verified with `git cat-file -t`
(returned `commit` in each case) in the stated repository, at delivery
time — per the standing rule that a Rust line's cited shas are verified to
exist before they are registered or booked:

| sha | repo | cat-file |
|---|---|---|
| `cc52e91f217f00165165ca60bc944c3c4fc81e86` | rust-r2-replay (also rust-r2-fix) | commit ✓ |
| `49c42a6e683a136269a08b06c381ae8116836f95` | rust-r2-fix | commit ✓ |
| `36044442041605a177a968bd6421b324ca59e192` | rust-r2-replay | commit ✓ |
| `43b42a75cf34059deea3104d70a56391a93c39ab` (R-1 fix) | rust-r2-replay | commit ✓ |
| `22db492c3fc20aea645822aac22f150a88cf322a` (composition) | rust-r2-replay | commit ✓ |
| `b56c298f18e353116391f5ea2fe9aaadefe774bb` (superseded draft, §0) | rust-r2-replay | commit ✓ |

The RESULT.md commit itself is the child of `22db492`; its sha is given in
the delivery report (it cannot cite itself here).

Blob ids (HEAD^ → HEAD at `22db492`), for the landing driver's pre-image
assertions: check_call.rs `d6a5e1b5…` → `6cce249b…`; check_method.rs
`07cc931c…` → `2ea617e3…`; lib.rs `164cb8ca…` → `a8db89f2…`; pipeline.rs
`3182ad15…` → `c54f799f…`. New-file blobs: check_assoc_helpers.rs
`f4e94ae6…`, check_std_ctor.rs `00cf6319…`, check_std_leaf.rs `b34cb525…`,
check_std_leaf2.rs `402f7c1d…`, check_std_leaf3.rs `f02be7ed…`,
tests_std_leaf.rs `8223d7b4…`.

## 9. Housekeeping and loose ends

- `Cargo.lock` (cargo-generated during verification; not tracked at head)
  was removed from the working tree and is not part of the delivery.
- `corpus/tests/.sla-cache/` in the working tree is a rerun by-product,
  untracked.
- Rerun raw evidence: `rerun/run_{stock,r2fix,replay}.tsv` + ~3400 per-run
  logs in `rerun/` (untracked, on disk).
- For the landing: the main session should expect exactly the 25 paths of
  commit `22db492`, plus this RESULT.md commit (which also renames
  `RESULT.md` → `RESULT-R1.md`). The post-landing census re-run (Kaggle,
  run/test level, box-caliber environment with `wrap/` present) remains
  the authoritative absolute-number event; §5.5's residual list is the
  diff baseline for it.
