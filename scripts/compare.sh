#!/bin/bash
# Slice byte-compare harness. RUN ON THE VENUE (never on the local VM).
# Builds the workspace (release), runs sla-oracle over fixtures/, and
# byte-compares against fixtures/expected/ (Zig plugin reference output).
# Usage: bash scripts/compare.sh
set -u
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
cargo build --release 2>&1 | tail -2 || exit 1
BIN=target/release/sla-oracle
mkdir -p results/actual
pass=0; fail=0
for src in fixtures/*.sla; do
  name=$(basename "$src" .sla)
  out="results/actual/$name.sa"
  if ! "$BIN" "$src" -o "$out" 2> "results/actual/$name.stderr"; then
    echo "FAIL(run)  $name"; fail=$((fail+1)); continue
  fi
  if cmp -s "$out" "fixtures/expected/$name.sa"; then
    echo "PASS       $name  ($(sha256sum "$out" | cut -c1-12))"
    pass=$((pass+1))
  else
    echo "FAIL(diff) $name"
    diff <(cat "fixtures/expected/$name.sa") <(cat "$out") | head -20
    fail=$((fail+1))
  fi
done
echo "compare: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
