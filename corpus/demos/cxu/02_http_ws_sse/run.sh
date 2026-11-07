#!/usr/bin/env bash
# D2 verification: builds server.sla + client.sla, starts the server,
# probes HTTP (curl), SSE (curl -N), and WebSocket (client.sla), then
# checks the server's own completion line. Exits non-zero on any miss.
set -u
cd "$(dirname "$0")"
export PATH=/content/sa_all/sci/zig-out/bin:$PATH

SA_PLUGIN_DEV=1 timeout 300 sa build-exe server.sla || exit 1
SA_PLUGIN_DEV=1 timeout 300 sa build-exe client.sla || exit 1

./server > server.log 2>&1 &
SRV=$!
cleanup() { kill "$SRV" 2>/dev/null; }
trap cleanup EXIT

# Readiness probe IS request #1: retry curl until the listener is up.
hello=""
for _ in $(seq 1 50); do
    hello=$(curl -s --max-time 5 http://127.0.0.1:18082/hello 2>/dev/null) && break
    sleep 0.2
done
echo "hello: $hello"
echo "$hello" | grep -q '"hello":"cxu"' || { echo "FAIL: /hello body"; exit 1; }

curl -sN --max-time 15 http://127.0.0.1:18082/events > events.out || { echo "FAIL: /events curl"; exit 1; }
for t in tick-1 tick-2 tick-3; do
    grep -q "data: $t" events.out || { echo "FAIL: /events missing $t"; exit 1; }
done
echo "events: $(tr '\n' ' ' < events.out)"

timeout 30 ./client || { echo "FAIL: ws client exit"; exit 1; }

wait "$SRV" || { echo "FAIL: server exit code"; exit 1; }
trap - EXIT
grep -q "d2 server ok" server.log || { echo "FAIL: server completion line"; exit 1; }
echo "d2 http/ws/sse ok"
