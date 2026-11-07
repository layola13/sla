#!/usr/bin/env bash
# D11 free-key service demo: builds server.sla and drives the scripted
# twelve-request sequence with curl, checking status + exact JSON body
# at every step (rotation, failure marking, health, rate limit, reset).
set -u
cd "$(dirname "$0")"
export PATH=/content/sa_all/sci/zig-out/bin:/content/sa_all/sa_plugin_sla/zig-out/bin:/tools/node/bin:$PATH
export SA_PLUGIN_DEV=1
rm -f server server.sa server.sa.bc
timeout 300 sa build-exe server.sla || { echo "D11 BUILD FAILED"; exit 1; }
timeout 60 ./server > server.log 2>&1 &
SRV=$!

fail() { echo "D11 CHECK FAILED: $1"; kill $SRV 2>/dev/null; exit 1; }
step() { # method path expected_status expected_body [data]
  local method="$1" path="$2" status="$3" want="$4" data="${5:-}"
  local out got body
  if [ -n "$data" ]; then
    out=$(curl -s --retry 25 --retry-connrefused --retry-delay 1 --retry-max-time 30 -X "$method" --data-binary "$data" -w $'\n%{http_code}' "http://127.0.0.1:18084$path")
  else
    out=$(curl -s --retry 25 --retry-connrefused --retry-delay 1 --retry-max-time 30 -X "$method" -w $'\n%{http_code}' "http://127.0.0.1:18084$path")
  fi
  got=$(printf '%s' "$out" | tail -1)
  body=$(printf '%s' "$out" | sed '$d')
  [ "$got" = "$status" ] || fail "$method $path status $got != $status (body: $body)"
  [ "$body" = "$want" ] || fail "$method $path body '$body' != '$want'"
  echo "ok $method $path -> $status $body"
}

# Like step, but without curl --retry: curl treats 429 as transient
# and would retry it, issuing extra requests that break the server's
# fixed request budget (and each retry is itself a request).
step_noretry() {
  local method="$1" path="$2" status="$3" want="$4" data="${5:-}"
  local out got body
  if [ -n "$data" ]; then
    out=$(curl -s -X "$method" --data-binary "$data" -w $'\n%{http_code}' "http://127.0.0.1:18084$path")
  else
    out=$(curl -s -X "$method" -w $'\n%{http_code}' "http://127.0.0.1:18084$path")
  fi
  got=$(printf '%s' "$out" | tail -1)
  body=$(printf '%s' "$out" | sed '$d')
  [ "$got" = "$status" ] || fail "$method $path status $got != $status (body: $body)"
  [ "$body" = "$want" ] || fail "$method $path body '$body' != '$want'"
  echo "ok $method $path -> $status $body"
}

KA='{"key":"key-a","pool":"free"}'
KB='{"key":"key-b","pool":"free"}'
KC='{"key":"key-c","pool":"free"}'
step GET /key 200 "$KA"
step GET /key 200 "$KB"
step POST /report 200 '{"reported":true}' 'key-a fail'
step POST /report 200 '{"reported":true}' 'key-a fail'
step GET /key 200 "$KC"
step GET /key 200 "$KB"
step GET /health 200 '{"up":2,"down":1}'
step GET /key 200 "$KC"
step GET /key 200 "$KB"
step_noretry GET /key 429 '{"error":"rate_limited"}'
step POST /reset 200 '{"reset":true}'
step GET /key 200 "$KA"

wait $SRV
rc=$?
[ $rc -eq 0 ] || { echo "D11 SERVER RC=$rc"; cat server.log; exit 1; }
grep -q "d11 freekey ok" server.log || { echo "D11 SERVER LOG MISSING OK"; cat server.log; exit 1; }
echo "d11 freekey demo ok"
