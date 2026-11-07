# INT-2 Zig-side probes (venue 3, 2026-11-07)

Environment: Kaggle venue 3 (hop 2202, hostname 66cf9c74eaf1),
`sa 0.1.8` (/content/sa_all/sci/zig-out/bin/sa),
`SA_PLUGIN_DEV=1`, `SA_STD_DIR=/content/sa_all/sci/sa_std`,
`LD_LIBRARY_PATH=/kaggle/working/content/tools/llvm14/usr/lib/x86_64-linux-gnu`,
sla tree /content/sa_all/sa_plugin_sla (head 6002b386).
Command form: `sa sla check <file>`.

| probe | content | result |
|---|---|---|
| p1_builtin_noimport.sla | NO imports; `println` with 1, 2 and 3 args; `str_eq("a","a")` | **rc=0** |
| p2_streq_types.sla | `str_eq(1, 2)` | **rc=1** `Type Check Error: ... checkExpr failed at call str_eq with 2 args (error.TypeMismatch)` |
| p3_extern_direct.sla | `@import "sa_std/io/print.sai"` + direct `sa_print_bytes("hi", 2)` | **rc=0** |
| fixtures/01_hello.sla | full fixture (check mode, @test body included) | **rc=0** |

Conclusions (dual-verified with Zig source):

- `println` / `str_eq` are checker name-specials, not contract externs:
  type_checker.zig `isPrintlnCall` arm (checks all args, returns void,
  any arity) and `isStrEqCall` arm (exactly 2 args, both must be
  string-like, returns boolean); name facts in lowering_rules.zig:4172
  (`isPrintlnCall`) / :4196 (`isStrEqCall`). They need no import and no
  registration, and they sit BEFORE user-function resolution, so a user
  `fn println` can never shadow them.
- Contracts (.sai) register only the `sa_*` extern names
  (type_checker.zig `loadContracts` :2105, `extern_funcs` arm :5067);
  no fixture calls those names directly — codegen emits them.
- Therefore the fixtures' Rust check-segment blocker is the Rust
  checker's missing name-special arms (a checker-lane mirroring gap),
  NOT the contract segment. Registering `println` as an extern would be
  a forgery: fixed arity (p1 proves 1/2/3 args all legal) and the
  extern bypass would accept `str_eq(1, 2)`, which Zig rejects (p2).
