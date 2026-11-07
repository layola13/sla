#!/bin/bash
# Staged timing comparison, same machine / same window. RUN ON THE VENUE.
# Zig side:  `sa sla build` end-to-end wall time x7 (median reported by
#           caller inspection of the log) + one SLA_PROFILE=1 `sa sla check`
#           run for the stage breakdown.
# Rust side: `sla-oracle` end-to-end x7 + one SLA_RS_PROFILE=1 run which
#           prints parse/check/codegen stage ms to stderr.
# Caliber: tiny inputs; both sides are startup-dominated.
# Phase-2 change (oracle line): bench inputs are docs/probes/e2e_*.sla --
# the fixtures all carry std-surface calls and cannot complete the Rust
# pipeline yet (INT-2), so timing them would measure fail-fast, not the
# pipeline. Probes run end-to-end on both sides.
# Numbers are slice-scale measurements only -- never extrapolate.
set -u
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
source /content/tools/env.sh
export SA_STD_DIR=/content/sa_all/sci/sa_std
cargo build --release 2>&1 | tail -1
mkdir -p results/bench_work && cp docs/probes/e2e_*.sla results/bench_work/
cd results/bench_work
for f in *.sla; do
  n=${f%.sla}
  echo "### $n zig_e2e_build_ms"
  for i in 1 2 3 4 5 6 7; do
    s=$(date +%s%N); sa sla build "$f" >/dev/null 2>&1; e=$(date +%s%N)
    echo $(( (e-s)/1000000 ))
  done
  echo "### $n zig_profile_check"
  SLA_PROFILE=1 sa sla check "$f" 2>&1 | grep 'sla-profile' | grep -E 'check (read source|source expand|parse|import expand|monomorphize|type check)|load contracts summary'
  echo "### $n rust_e2e_ms"
  for i in 1 2 3 4 5 6 7; do
    s=$(date +%s%N); ../../target/release/sla-oracle "$f" -o "$n.rs.sa" 2>/dev/null; e=$(date +%s%N)
    echo $(( (e-s)/1000000 ))
  done
  echo "### $n rust_profile"
  SLA_RS_PROFILE=1 ../../target/release/sla-oracle "$f" -o "$n.rs.sa" 2>&1 >/dev/null | grep 'stage'
done
