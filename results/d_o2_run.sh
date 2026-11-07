#!/bin/bash
# D-O2 closure run: build-mode @test pruning parity. RUN ON THE VENUE only.
# Evidence lands in results/p2/d_o2/ (continuation of the p2 archive; the
# original p2 baseline evidence is NOT wiped).
set -u
export PATH="/root/.cargo/bin:$PATH"
cd /root/sla-plugin-rust
source /content/tools/env.sh
export SA_STD_DIR=/content/sa_all/sci/sa_std
SA=/kaggle/working/content/sa_all/sci/zig-out/bin/sa
OUT=results/p2/d_o2
rm -rf "$OUT"
mkdir -p "$OUT"
{
  echo "== sa version =="
  "$SA" --version
  echo "== cargo build --release =="
  cargo build --release 2>&1 | tail -3
  echo "== cargo test --workspace (summary lines) =="
  cargo test --workspace 2>&1 | grep -E "^(test result|running|error)"
  echo "== cargo test -p sla-oracle (tail) =="
  cargo test -p sla-oracle 2>&1 | tail -8
  echo "== check_file_lines =="
  bash check_file_lines.sh
  echo "== compare.sh (fixtures vs goldens) =="
  bash scripts/compare.sh 2>&1
} > "$OUT/build_test.log" 2>&1

BIN=target/release/sla-oracle
for p in docs/probes/e2e_a.sla docs/probes/e2e_b.sla docs/probes/e2e_c.sla; do
  n=$(basename "$p" .sla)
  mkdir -p "$OUT/$n/run1/dump" "$OUT/$n/run2/dump" "$OUT/$n/fullmode/dump" \
           "$OUT/$n/zigwork" "$OUT/$n/zigwork2"
  # Rust, default (build mode), twice for the determinism self-check.
  "$BIN" "$p" -o "$OUT/$n/run1/out.sa" --dump-boundaries "$OUT/$n/run1/dump" \
    > "$OUT/$n/run1/stdout.txt" 2> "$OUT/$n/run1/stderr.txt"
  echo $? > "$OUT/$n/run1/rc"
  SLA_RS_PROFILE=1 "$BIN" "$p" -o "$OUT/$n/run2/out.sa" --dump-boundaries "$OUT/$n/run2/dump" \
    > "$OUT/$n/run2/stdout.txt" 2> "$OUT/$n/run2/stderr.txt"
  echo $? > "$OUT/$n/run2/rc"
  # Control: --no-build-mode must keep the full @test body (old behavior).
  "$BIN" --no-build-mode "$p" -o "$OUT/$n/fullmode/out.sa" --dump-boundaries "$OUT/$n/fullmode/dump" \
    > "$OUT/$n/fullmode/stdout.txt" 2> "$OUT/$n/fullmode/stderr.txt"
  echo $? > "$OUT/$n/fullmode/rc"
  # Zig plugin, this window, twice (window-drift control for e2e_c).
  cp "$p" "$OUT/$n/zigwork/"
  ( cd "$OUT/$n/zigwork" \
    && "$SA" sla build "$(basename "$p")" > stdout.txt 2> stderr.txt; \
    echo $? > rc; ls -la >> ls.txt 2>&1 )
  cp "$p" "$OUT/$n/zigwork2/"
  ( cd "$OUT/$n/zigwork2" \
    && "$SA" sla build "$(basename "$p")" > stdout.txt 2> stderr.txt; \
    echo $? > rc; ls -la >> ls.txt 2>&1 )
  # Diffs and hashes for the verdict.
  diff "$OUT/$n/zigwork/$n.sa" "$OUT/$n/run1/out.sa" > "$OUT/$n/diff_vs_zig.txt" 2>&1
  echo $? > "$OUT/$n/diff_vs_zig.rc"
  diff "$OUT/$n/run1/out.sa" "$OUT/$n/run2/out.sa" > "$OUT/$n/diff_run1_run2.txt" 2>&1
  echo $? > "$OUT/$n/diff_run1_run2.rc"
  diff "$OUT/$n/zigwork/$n.sa" "$OUT/$n/zigwork2/$n.sa" > "$OUT/$n/diff_zig_zig2.txt" 2>&1
  echo $? > "$OUT/$n/diff_zig_zig2.rc"
  sha256sum "$OUT/$n/zigwork/$n.sa" "$OUT/$n/zigwork2/$n.sa" \
            "$OUT/$n/run1/out.sa" "$OUT/$n/run2/out.sa" "$OUT/$n/fullmode/out.sa" \
    > "$OUT/$n/sha256.txt"
done
echo DONE > "$OUT/FINISHED"
