#!/usr/bin/env bash
# D9 VM tool surface demo: one binary, three 16_policy scenarios.
#  A allow-list: both tools work; the second read is allowed too.
#  B deny: the first tool call surfaces status 10; program exits 42.
#  C ask + one-time tokens (SA_POLICY_TOKENS_JSON): each file opens
#    exactly once; the deliberate second read of lib.ts is denied
#    because its token was consumed. The audit log must show the two
#    token-sourced allows.
set -u
cd "$(dirname "$0")"
export PATH=/content/sa_all/sci/zig-out/bin:/content/sa_all/sa_plugin_sla/zig-out/bin:$PATH
export SA_PLUGIN_DEV=1
export SA_TREE_SITTER_DIR=/content/sa_all/sci/artifacts/tree_sitter
rm -f main main.sa main.sa.bc main.test main.test.sa audit_a.jsonl audit_c.jsonl
timeout 300 sa build-exe main.sla || { echo "D9 BUILD FAILED"; exit 1; }

BASE='"time":{"mode":"allow"},"random":{"mode":"allow"}'

echo "--- scenario A: fs allow-list ---"
SA_POLICY_JSON="{\"permissions\":{\"fs\":{\"mode\":\"allow-list\",\"read\":[\"lib.ts\",\"notes.txt\"]},$BASE}}" \
SA_AUDIT_FILE=audit_a.jsonl \
  timeout 60 ./main > out_a.txt 2>&1
rc=$?
cat out_a.txt
[ $rc -eq 0 ] && grep -q "find_definition ok" out_a.txt && grep -q "read_file ok" out_a.txt \
  && grep -q "second read allowed" out_a.txt && grep -q "d9 vm tools ok" out_a.txt \
  || { echo "D9 SCENARIO A FAILED rc=$rc"; exit 1; }

echo "--- scenario B: fs deny ---"
SA_POLICY_JSON="{\"permissions\":{\"fs\":{\"mode\":\"deny\"},$BASE}}" \
  timeout 60 ./main > out_b.txt 2>&1
rc=$?
cat out_b.txt
[ $rc -eq 42 ] && grep -q "tool denied at first read (10)" out_b.txt \
  || { echo "D9 SCENARIO B FAILED rc=$rc"; exit 1; }

echo "--- scenario C: fs ask + one-time tokens ---"
SA_POLICY_JSON="{\"permissions\":{\"fs\":{\"mode\":\"ask\"},$BASE}}" \
SA_POLICY_TOKENS_JSON='[{"category":"fs","key":"read:lib.ts"},{"category":"fs","key":"read:notes.txt"}]' \
SA_AUDIT_FILE=audit_c.jsonl \
  timeout 60 ./main > out_c.txt 2>&1
rc=$?
cat out_c.txt
[ $rc -eq 0 ] && grep -q "find_definition ok" out_c.txt && grep -q "read_file ok" out_c.txt \
  && grep -q "second read denied" out_c.txt && grep -q "d9 vm tools ok" out_c.txt \
  || { echo "D9 SCENARIO C FAILED rc=$rc"; exit 1; }
tok=$(grep -c '"source":"token"' audit_c.jsonl || true)
echo "token-sourced audit allows: $tok"
[ "$tok" -eq 2 ] || { echo "D9 AUDIT CHECK FAILED"; cat audit_c.jsonl; exit 1; }

echo "d9 vm tools demo ok"
