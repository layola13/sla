# Slice fixtures — provenance

Inputs copied read-only from the original repo
`~/workspace/local-sa/sa_plugin_sla/demos/basic/<dir>/main.sla`:

| fixture | source demo | bytes |
|---|---|---|
| 01_hello.sla | demos/basic/01_hello/main.sla | 287 |
| 02_arith.sla | demos/basic/02_arith/main.sla | 729 |
| 03_if_else.sla | demos/basic/03_if_else/main.sla | 429 |
| 04_while_sum.sla | demos/basic/04_while_sum/main.sla | 522 |
| 07_functions.sla | demos/basic/07_functions/main.sla | 544 |

Expected outputs (`fixtures/expected/*.sa`) were produced by the **Zig
plugin on the Kaggle venue** (venue tree `/content/sa_all/sa_plugin_sla`,
host `sa 0.1.8`, `SA_PLUGIN_DEV=1`, `SA_STD_DIR=/content/sa_all/sci/sa_std`)
via `sa sla build <file>.sla`, on 2026-11-07, and pulled back byte-exact
(sha256 verified after transfer):

| expected | sha256 |
|---|---|
| 01_hello.sa | 0d6375acf0acca14a8824b8f96a91da3e8e16dba6bb335f46e03b06367817b26 |
| 02_arith.sa | 0b13f453f7221cbe8ff9efdd8497445cc05e18484c4afa1df8c1e49ede7a8128 |
| 03_if_else.sa | 5a92ee8240d1a1660bed6ba2be93af5e840298a8d0e26e5faf87c0f3fcf064a7 |
| 04_while_sum.sa | 60f814780a050c896d21fff3e46d04b1cb4bfae3164f0e0e7b941dd0768937dd |
| 07_functions.sa | 774e41d678658ff8a5308201c2349a1767b5bf4748971fc6fab1b5ba018d2dcc |

Note: the venue plugin tree is the venue's checked-out head, not byte-wise
the local `~/workspace/local-sa` tree; the compare is self-consistent
(same inputs, venue Zig output as reference).

## Regeneration — 2026-11-26 (task 172, parity reconciliation)

The five expected outputs were regenerated wholesale from the current
Zig plugin with the same recipe as above (`sa sla build <file>.sla`,
`SA_PLUGIN_DEV=1`, `SA_STD_DIR=<sci tree>/sa_std`): sci `500477e5`,
sa_plugin_sla master `50358f93` (its `src/` is unchanged through
`56ab2c53`; the intervening commits touch only `sla_std/`), ReleaseFast
builds with `env -u SA_PLUGIN_DEV`, plugin installed into an isolated
HOME (installed `libsla.so` 39,676,856 B), caches cleared,
`SLA_NO_FRESHNESS_CACHE=1`.

| expected | sha256 |
|---|---|
| 01_hello.sa | 90a79e2eadd14856135da3f99fcc06ac8d5e2451c9f6514aabdd2b3c333345bf |
| 02_arith.sa | 6877bbdfd578e9968b8c6a53cfefdcc82369e10cdb94584813d5cc00ed949d0c |
| 03_if_else.sa | bba6af2088a52b9fac271f7a25ecf3657c14ee6d86daf0b27478c6f5b988ad0c |
| 04_while_sum.sa | b70e88000923d3e40087642a7acf4a1ec2c312510778dffbf6abf6c422722ae4 |
| 07_functions.sa | ff2bfa333dbd0701f6038efd581fd13740758f1f8e8a65148a03165b5066f4b0 |

Why the bytes moved: the 2026-11-07 outputs had drifted from the
plugin in four change classes, each traced to a landed, gated change
in sa_plugin_sla (full attribution in the reconciliation record):
narrow-integer width canonicalization at binding/consumption
boundaries (DEF-TEXT-SHIFT-WRAP form G, `e4e2d9e2`, plus follow-ons),
scope-exit retirement order/set (FB-016/020 declaration-order
cleanup, `b03d45ac`; DEF-REFCELL-PAYLOAD-ACCOUNTING tail-param
release, `34e192d5`), `@test` body materialization in build mode
(task 112 B+C, `0b90f8d4`), and the A1 minimum-import prune
(R441 form B, `6e40419e`) with the conditional `ffi.sa` import
(CLOSING clusters, 2026-11-17). The Rust codegen does not yet mirror
those four behaviours, so `fixture_parity` is expected red on
01/02/03/04/07 until the Rust follow-ups land; the residuals are
exactly those classes and nothing else.
