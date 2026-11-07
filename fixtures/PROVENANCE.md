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
