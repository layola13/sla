# Phase 3 — INT21 name-special arms (delivery report)

Lane: INT21 (checker name-special arms; the INT-2 cross-lane
handoff INT2-1). Venue: Kaggle venue 3 (hop 2202, hostname
66cf9c74eaf1), tree /root/sla-plugin-rust synced per-path with
md5 verification (crates/sla-checker 30/30 files identical).
Zig truth source: venue head 6002b386, `sa 0.1.8`,
type_checker.zig 10,917 lines (the local reference tree's
6,098-line copy is stale and was NOT used for line numbers) +
33 real-plugin probes (`sa sla check`, /tmp/int21/p_*.sla,
rc + diagnostic text recorded below).

## 1. Census: the name-special family (venue head)

The `call_expr` arm dispatches in this order: callee (IIFE) →
chan-method probe → **pre-associated bare specials** (:8212-
8436: `Some`/`None`/`Ok`/`Err` constructors, `monitor`,
`ptr__null`, `future__*`, `poll__*`, `RAW_WAKER_*`/`WAKER_*`,
`va_next_*`, `ptr__read_volatile`) → associated-target block
(:8438-8803) → **post-associated bare block (:8805-8895)** →
closure/fn-ptr locals → overloads → user functions → imported
signatures → struct constructors → receiver-inferred method
leaves → `sum` (:9811) → externs (:9833) → `stack_alloc`/
`panic` builtins (:9838) → macros → imported macros →
`Undefined call: {name}` fallback (:9982).

INT2-1's定层 and INT-2 §7 scope the missing family to the
post-associated block. That block is mirrored **in full**:

| arm | venue | arity | argument gate | returns | consume/borrow effect |
|---|---|---|---|---|---|
| `vec` | :8805 | ≥1 (0 → InvalidArgsCount, setError "vec requires at least one element") | every arg checked; all `typesEqual` to arg[0] else TypeMismatch | `Vec<elem of arg[0]>` | args checked only |
| `len` | :8822 | exactly 1 (else InvalidArgsCount, setError "len call arity mismatch: {n}") | arg checked; **type NOT inspected** (`len(42)` rc=0) | `usize` | arg checked only (String reusable) |
| `str_eq` | :8832 | exactly 2 (else InvalidArgsCount) | both string-like (lowering_rules:5201: peel borrow/pointer → raw_ptr / array / `String`) else TypeMismatch | `boolean` | args checked only (String reusable twice, probe) |
| `format` | :8841 | ≥1 AND args[0] is a string **literal node** (else InvalidArgsCount — a String variable fails) | args[1..] checked | `String` | args checked only |
| `hash` | :8848 | exactly 1 | `typeIsHash` (:4807) else TypeMismatch | `u64` | arg checked only |
| `debug` | :8854 | exactly 1 | `typeIsDebug` (:4819) else TypeMismatch | `String` | arg checked only |
| `println` | :8860 | ANY, incl. 0 | every arg checked, no type gate | `void` | args checked only, never consumed (probe: String survives twice) |
| `panic`/`panic_msg` | :8869 | any | args checked | `void` | already mirrored by this crate's builtin step — not duplicated |
| `std__panic__catch_unwind` / `panic::catch_unwind` | :8878 | exactly 1 | arg checks as closure with 0 params; arg node must be a closure literal whose body is a direct panic-name call, else TypeMismatch (setError "catch_unwind currently only supports direct panic bodies") | `Result<i32, i32>` | closure checked in its own frame |

Position: all of the above sit BEFORE closure/fn-ptr locals,
user functions, externs and macros — unshadowable. Venue
probes: user `fn println(x: i32) -> i32` + call `println(1, 2)`
→ rc=0; user `fn len(a: i32, b: i32)` + call `len(1)` → rc=0.

`typeIsHash` / `typeIsDebug` (the only non-trivial gates):
base via lowering_rules `typeHasNamedDeriveBase` — primitive:
hashable = NOT (raw_ptr | void | f32 | f64 | float);
debug-base = Copy-value = NOT (raw_ptr | void), floats
INCLUDED (probes: `hash(1.5)` rc=1, `debug(1.5)` rc=0;
`hash("a")`/`debug("a")` rc=1 — string literals are raw_ptr).
User-defined std-owner names (`userDefinedStdOwnerIsNonCopy`:
Vec/GrowVec/VecDeque/String/Box/Rc/Arc/HashMap/BTreeMap/
HashSet/BTreeSet/RefCell/Mutex/RwLock/JoinHandle) decide
false at the base (probes: `hash(format_result)` rc=1,
`debug(string_value)` rc=1). Other user types: struct decl
must carry the derive (case-insensitive `structHasDerive`),
be non-opaque and non-union, and ALL fields must recurse
(probes: `@derive(hash, debug)` struct rc=0; no-derive struct
rc=1 both arms; `@derive(hash)` struct with a `String` field
rc=1). All other shapes (tuple/array/pointer/borrow/closure)
are false. One porting note: Zig's field recursion is
unbounded; the port carries a visiting set (the
`type_is_eq_ord` precedent in check_binary.rs), so a
self-referential derive struct decides false instead of
recursing forever.

The pre-associated family (:8212-8436) is censused, not
mirrored — registered as D-C22 (coverage gap, phase 4):
its blocking condition is a corpus/fixture case reaching
those names in call form and falling to the Rust
`Undefined call` fallback; no current fixture or e2e probe
does (Option/Result construction travels the enum-literal /
pattern routes in this pipeline; the `None` identifier form
and `Option::None` associated form are already handled).

## 2. What was built

| piece | home | mirrors |
|---|---|---|
| The 8 arms + `type_is_hash` / `type_is_debug` + std-owner / primitive bases | `crates/sla-checker/src/check_builtin.rs` (NEW, 398 lines) | venue type_checker.zig:8805-8895, :4807/:4819; lowering_rules.zig:4301-4336, :5201, :6563, :6635, :6651, :6802, :4972 |
| `is_string_like_type` | `crates/sla-checker/src/types_util.rs` | lowering_rules.zig:5201 |
| Dispatch hook (after associated block, before all other resolution) | `crates/sla-checker/src/check_call.rs` (+9 lines) | venue dispatch position |
| Tests: 28, parse→check pipeline form, incl. all 5 fixture sources verbatim | `crates/sla-checker/src/tests_phase3d.rs` (NEW, 349 lines) | the 33 venue probes, 1:1 |
| lib.rs wiring | `crates/sla-checker/src/lib.rs`: added `mod check_builtin;` and `#[cfg(test)] mod tests_phase3d;` | — (lane-authorized) |

No Cargo.toml touched. No other crate's source touched.
Oracle was run, not modified.

## 3. Fixture re-run accounting (oracle, build mode, venue)

Environment: `SA_STD_DIR=/content/sa_all/sci/sa_std`. Death
point before this lane (INT-2 §3): check, `println` arm, for
all five. After: **all five complete the full pipeline rc=0**.

| fixture | before | after: death point | output sha256 vs expected |
|---|---|---|---|
| 01_hello | check: println (rc=5) | — (completes) | **byte-identical** `0d6375acf0acca14a8824b8f96a91da3e8e16dba6bb335f46e03b06367817b26` |
| 02_arith | check: println (rc=5) | — (completes) | same length (3493 B), ONE cleanup-order pair swap (see below) |
| 03_if_else | check: println (rc=5) | — (completes) | **byte-identical** `5a92ee8240d1a1660bed6ba2be93af5e840298a8d0e26e5faf87c0f3fcf064a7` |
| 04_while_sum | check: println (rc=5) | — (completes) | **byte-identical** `60f814780a050c896d21fff3e46d04b1cb4bfae3164f0e0e7b941dd0768937dd` |
| 07_functions | check: println (rc=5) | — (completes) | same length (1909 B), ONE cleanup-order pair swap (see below) |

The 02/07 residual, converged: in `arith_values` the exit
cleanup reads `!a !b !c !d` (Rust) vs `!b !a !c !d` (Zig);
in `add`, `!a !b` vs `!b !a`. Byte counts and line multisets
are identical; the ONLY difference is the release order of
the first parameter pair. This is D-C6 (Zig `checkBlock`
exit iterates `scope.symbols` StringHashMap — hash order,
neither declaration nor reverse) manifesting end-to-end for
the first time; registered as **D-C23** in
docs/divergences/checker.md with the disposition left to the
coordinator (replicating Zig's hash layout vs pushing Zig to
declaration order). It is NOT an INT2-1 residual: the check
segment is fully unblocked (fixture 01 also passes the
oracle in `--no-build-mode`, rc=0 — the latent `str_eq`
second blocker is gone too).

Regression control: oracle e2e_b / e2e_c outputs unchanged —
sha256 `8623e8d0b1326cddb50b4c2b23fd1de36412ff06eec866e2bf2a7862851c831e`
/ `fe1902818c4a175e630cbe0d362f215693b0aeb7efbd6efdf7a5109d59b47ada`,
byte-identical to the phase-2/3 records.

## 4. Side items (INT2-2 / INT2-3) — rulings

- **INT2-2 (`ExternSig.return_fallible`): NOT blocking.**
  Zig readers are codegen-only (codegen.zig:2989/19043,
  sab_codegen.zig); check-time behavior never reads it; no
  fixture calls a fallible extern from source. The field
  addition itself is one line, but its construction sites
  (`sla-plugin/src/contracts.rs::extern_sig_of`, the
  sla-oracle shim literal) are outside this lane's
  jurisdiction — adding the field unilaterally would break
  both crates' compilation and regress the workspace.
  Precise trigger for the coordinator to land it atomically
  (field + both sites): a fixture/corpus case that calls a
  fallible extern from source and needs the flag plumbed
  through the checker to codegen. Recorded in contracts.md.
- **INT2-3 (`layout_defines` table): NOT blocking anything.**
  Venue full-tree grep proves Zig's `layout_defines` has NO
  reader (written at loadContracts :6064, deinit only) — it
  is inert metadata; fixture 01's 323 defines remain visible
  in the Rust `LoadReport`. No table added (an unread
  registry would be motion without effect); mirror it when
  Zig grows a first reader. Recorded in contracts.md.

## 5. Tests (venue)

- `cargo test -p sla-checker` = **146 passed / 0 failed**
  (baseline 118 + 28 new in tests_phase3d.rs).
- `cargo test --workspace --no-fail-fast` = **264 passed /
  1 failed**. The single failure is the expected INT2-1 pin
  expiring: `sla-plugin --test contracts_pipeline ::
  println_remains_blocked_by_missing_checker_builtin_arm` —
  INT-2's own registration states the pin "must be updated
  when the arms land"; the file is outside this lane's
  jurisdiction (sla-plugin lane / coordinator action). Every
  other suite is green, including contracts_pipeline's other
  two tests, fixture_parity 5/5, and SAB driver_e2e 11/11.
  Against this lane's scope the baseline is not regressed:
  checker 118→146, all previously-green suites still green.
- File-line discipline: check_builtin.rs 398, tests_phase3d.rs
  349, check_call.rs 657 — all ≤700. (The workspace line
  check's one OVER entry, sla-codegen-sab/tests/driver_e2e.rs
  at 765, is pre-existing and another lane's.)

## 6. Divergences and feedback

- INT2-1: **CLOSED** (closure note in
  docs/divergences/contracts.md).
- New in docs/divergences/checker.md: **D-C22** (a,
  pre-associated bare-special census remainder, phase 4) and
  **D-C23** (a, the 02/07 cleanup-order residual = D-C6
  manifesting end-to-end; coordinator disposition).
- Feedback slots FB-029..FB-032: **none issued** — every arm's
  Zig behavior proved coherent under probes (len's missing
  type gate, debug-includes-float vs hash-excludes-float,
  String not hashable/debug-able are consistent design, not
  defects); D-C23 is the on-register D-C6 candidate acquiring
  end-to-end evidence, not a new candidate. Slots returned
  unused.
