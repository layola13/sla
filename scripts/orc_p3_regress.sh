#!/bin/bash
# ORC phase-3 stage A: full-pipeline byte-parity regression re-run.
# RUN ON THE VENUE ONLY (Kaggle venue 3, hop 2202). Never on the local VM.
#
# For each e2e probe (docs/probes/e2e_{a,b,c}.sla):
#   - Rust side: sla-oracle (default build mode, D-O2) run1 + run2
#     (determinism pair), with boundary dumps;
#   - Zig side: the venue's installed current-head plugin via
#     `sa sla build`, in two independent work dirs (zigwork / zigwork2);
#   - sha256 of every artifact + diffs (rust-vs-zig, run-vs-run, zig-vs-zig)
#     land under results/p3/regress/<name>/.
# Phase-2 reference shas (results/p2/d_o2, REPORT section 9.3):
#   e2e_b zig+rust 8623e8d0b1326cddb50b4c2b23fd1de36412ff06eec866e2bf2a7862851c831e
#   e2e_c zig+rust fe1902818c4a175e630cbe0d362f215693b0aeb7efbd6efdf7a5109d59b47ada
#   e2e_a zig 87adac2002a05cb7eec6054feaf6b44f3fa6c0e3e0af8739b85cf2e205f98890
#         rust 38c71fab450628b3b1e31e5dd50e62b419829c11b27ea03d2f0dfb968c81ff26
#         (sole diff = D-O1 / FB-020 return-cleanup order)
# Usage: bash scripts/orc_p3_regress.sh
set -u
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
source /content/tools/env.sh
export SA_STD_DIR=/content/sa_all/sci/sa_std
OUT=results/p3/regress
rm -rf "$OUT"
mkdir -p "$OUT"
{
  echo "== provenance =="
  date -u +"utc %Y-%m-%dT%H:%M:%SZ"
  hostname
  echo "sa: $(sa --version 2>&1)"
  echo "sci head: $(git -C /content/sa_all/sci rev-parse HEAD)"
  echo "sla head: $(git -C /content/sa_all/sa_plugin_sla rev-parse HEAD)"
  echo "rustc: $(rustc --version)"
  echo "installed plugin: /root/.local/share/sa_plugins/installed/sla/current/libsla.so"
  sha256sum /root/.local/share/sa_plugins/installed/sla/current/libsla.so
  stat -c 'size %s mtime %y' /root/.local/share/sa_plugins/installed/sla/current/libsla.so
  echo "== cargo build --release =="
  cargo build --release 2>&1 | tail -2
} > "$OUT/provenance.txt" 2>&1

BIN=target/release/sla-oracle
for p in docs/probes/e2e_a.sla docs/probes/e2e_b.sla docs/probes/e2e_c.sla; do
  n=$(basename "$p" .sla)
  mkdir -p "$OUT/$n/run1/dump" "$OUT/$n/run2/dump" "$OUT/$n/zigwork" "$OUT/$n/zigwork2"
  "$BIN" "$p" -o "$OUT/$n/run1/out.sa" --dump-boundaries "$OUT/$n/run1/dump" \
    > "$OUT/$n/run1/stdout.txt" 2> "$OUT/$n/run1/stderr.txt"
  echo $? > "$OUT/$n/run1/rc"
  SLA_RS_PROFILE=1 "$BIN" "$p" -o "$OUT/$n/run2/out.sa" --dump-boundaries "$OUT/$n/run2/dump" \
    > "$OUT/$n/run2/stdout.txt" 2> "$OUT/$n/run2/stderr.txt"
  echo $? > "$OUT/$n/run2/rc"
  cp "$p" "$OUT/$n/zigwork/"
  ( cd "$OUT/$n/zigwork" \
    && sa sla build "$(basename "$p")" > stdout.txt 2> stderr.txt; \
    echo $? > rc; ls -la > ls.txt 2>&1 )
  cp "$p" "$OUT/$n/zigwork2/"
  ( cd "$OUT/$n/zigwork2" \
    && sa sla build "$(basename "$p")" > stdout.txt 2> stderr.txt; \
    echo $? > rc; ls -la > ls.txt 2>&1 )
  sha256sum "$p" "$OUT/$n/run1/out.sa" "$OUT/$n/run2/out.sa" \
    "$OUT/$n/zigwork/$n.sa" "$OUT/$n/zigwork2/$n.sa" > "$OUT/$n/sha256.txt" 2>&1
  diff "$OUT/$n/run1/out.sa" "$OUT/$n/zigwork/$n.sa" > "$OUT/$n/diff_vs_zig.txt" 2>&1
  echo $? > "$OUT/$n/diff_vs_zig.rc"
  diff "$OUT/$n/run1/out.sa" "$OUT/$n/run2/out.sa" > "$OUT/$n/diff_run1_run2.txt" 2>&1
  echo $? > "$OUT/$n/diff_run1_run2.rc"
  diff "$OUT/$n/zigwork/$n.sa" "$OUT/$n/zigwork2/$n.sa" > "$OUT/$n/diff_zig_zig2.txt" 2>&1
  echo $? > "$OUT/$n/diff_zig_zig2.rc"
done
echo DONE > "$OUT/FINISHED"
