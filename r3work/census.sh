#!/bin/bash
# R-3 census harness: standalone `sla check` over every .sla in corpus/tests + corpus/demos.
# Usage: census.sh <sla-binary> <corpus-dir> <outdir> <tag>
# Records: relpath, rc, kind (first diagnostic name found), first output line.
set -u
BIN="$1"; CORPUS="$2"; OUT="$3"; TAG="$4"
mkdir -p "$OUT/logs_$TAG"
export SA_PLUGIN_DEV=1
export SLA_NO_FRESHNESS_CACHE=1
export SA_SLA_FIXTURE_DIR="$CORPUS/tests/import_fixtures"
export LD_LIBRARY_PATH="$HOME/workspace/local-tools/llvm14/extracted/usr/lib/x86_64-linux-gnu"
export PATH="$HOME/workspace/local-sa/sci/zig-out/bin:$PATH"
RES="$OUT/census_${TAG}.tsv"
: > "$RES"
cd "$CORPUS" || exit 1
find tests demos -name '*.sla' | sort > "$OUT/files_${TAG}.txt"
while IFS= read -r rel; do
  log="$OUT/logs_$TAG/$(echo "$rel" | tr '/' '_').log"
  d=$(dirname "$rel")
  ( cd "$CORPUS/$d" && timeout 60 "$BIN" check "$CORPUS/$rel" ) > "$log" 2>&1
  rc=$?
  kind=$(grep -m1 -oP 'error\.\K[A-Za-z]+' "$log" || true)
  if [ -z "$kind" ]; then kind=$(grep -m1 -oP '^\K[A-Z][A-Za-z]+(?=:)' "$log" || true); fi
  firstline=$(head -n 1 "$log" | head -c 200)
  printf '%s\t%s\t%s\t%s\n' "$rel" "$rc" "$kind" "$firstline" >> "$RES"
done < "$OUT/files_${TAG}.txt"
echo "DONE $TAG $(wc -l < "$RES") files"
