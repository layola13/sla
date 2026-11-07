#!/bin/bash
# Oracle phase-2 run: probe parity + fixture INT-2 attempts + tests.
# RUN ON THE VENUE only. Detached-safe: all output lands in results/p2/.
set -u
export PATH="/root/.cargo/bin:$PATH"
cd /root/sla-plugin-rust
source /content/tools/env.sh
export SA_STD_DIR=/content/sa_all/sci/sa_std
SA=/kaggle/working/content/sa_all/sci/zig-out/bin/sa
OUT=results/p2
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
  cargo test -p sla-oracle 2>&1 | tail -6
  echo "== check_file_lines =="
  bash check_file_lines.sh
  echo "== compare.sh (fixtures vs goldens) =="
  bash scripts/compare.sh 2>&1
} > "$OUT/build_test.log" 2>&1

BIN=target/release/sla-oracle
for p in docs/probes/e2e_a.sla docs/probes/e2e_b.sla docs/probes/e2e_c.sla; do
  n=$(basename "$p" .sla)
  mkdir -p "$OUT/$n/run1/dump" "$OUT/$n/run2/dump" "$OUT/$n/zigwork"
  "$BIN" "$p" -o "$OUT/$n/run1/out.sa" --dump-boundaries "$OUT/$n/run1/dump" \
    > "$OUT/$n/run1/stdout.txt" 2> "$OUT/$n/run1/stderr.txt"
  echo $? > "$OUT/$n/run1/rc"
  SLA_RS_PROFILE=1 "$BIN" "$p" -o "$OUT/$n/run2/out.sa" --dump-boundaries "$OUT/$n/run2/dump" \
    > "$OUT/$n/run2/stdout.txt" 2> "$OUT/$n/run2/stderr.txt"
  echo $? > "$OUT/$n/run2/rc"
  cp "$p" "$OUT/$n/zigwork/"
  ( cd "$OUT/$n/zigwork" \
    && "$SA" sla build "$(basename "$p")" > stdout.txt 2> stderr.txt; \
    echo $? > rc; ls -la >> ls.txt 2>&1 )
done
for f in fixtures/*.sla; do
  n=$(basename "$f" .sla)
  mkdir -p "$OUT/fx_$n/dump"
  "$BIN" "$f" -o "$OUT/fx_$n/out.sa" --dump-boundaries "$OUT/fx_$n/dump" \
    > "$OUT/fx_$n/stdout.txt" 2> "$OUT/fx_$n/stderr.txt"
  echo $? > "$OUT/fx_$n/rc"
done
echo DONE > "$OUT/FINISHED"
