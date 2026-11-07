#!/usr/bin/env bash
# D10 static serving demo: builds server.sla, starts it, and verifies
# six requests with curl — status codes, content types, cache headers
# and byte-exact bodies — including a 404 and a raw traversal attempt
# (curl --path-as-is). The server exits by itself after six requests.
set -u
cd "$(dirname "$0")"
export PATH=/content/sa_all/sci/zig-out/bin:/content/sa_all/sa_plugin_sla/zig-out/bin:/tools/node/bin:$PATH
export SA_PLUGIN_DEV=1
rm -f server server.sa server.sa.bc
timeout 300 sa build-exe server.sla || { echo "D10 BUILD FAILED"; exit 1; }
timeout 60 ./server > server.log 2>&1 &
SRV=$!
# No TCP readiness probe: a bare connect would be accepted as a
# request by the server. The first check uses curl retries instead.

fail() { echo "D10 CHECK FAILED: $1"; kill $SRV 2>/dev/null; exit 1; }

check() { # path expected_status expected_ctype expected_cache file_to_diff|none extra_curl_args...
  local path="$1" status="$2" ctype="$3" cache="$4" diff_file="$5"; shift 5
  local hdr body
  hdr=$(mktemp); body=$(mktemp)
  local got
  got=$(curl -s --path-as-is --retry 25 --retry-connrefused --retry-delay 1 --retry-max-time 30 -o "$body" -D "$hdr" -w "%{http_code}" "http://127.0.0.1:18083$path" "$@")
  [ "$got" = "$status" ] || fail "$path status $got != $status"
  grep -qi "^content-type: $ctype" "$hdr" || fail "$path content-type: $(grep -i '^content-type' "$hdr")"
  if [ "$cache" != "none" ]; then
    grep -qi "^cache-control: $cache" "$hdr" || fail "$path cache-control: $(grep -i '^cache-control' "$hdr")"
  fi
  if [ "$diff_file" != "none" ]; then
    cmp -s "$body" "$diff_file" || fail "$path body differs from $diff_file"
  fi
  rm -f "$hdr" "$body"
  echo "ok $path -> $status $ctype"
}

check "/" 200 "text/html" "no-cache" www/index.html
check "/index.html" 200 "text/html" "no-cache" www/index.html
check "/app.js" 200 "text/javascript" "public, max-age=3600" www/app.js
check "/style.css" 200 "text/css" "public, max-age=3600" www/style.css
check "/missing.txt" 404 "text/plain" "none" none
check "/../secret.txt" 404 "text/plain" "none" none

wait $SRV
rc=$?
[ $rc -eq 0 ] || { echo "D10 SERVER RC=$rc"; cat server.log; exit 1; }
grep -q "d10 static ok" server.log || { echo "D10 SERVER LOG MISSING OK"; cat server.log; exit 1; }
echo "d10 static demo ok"
