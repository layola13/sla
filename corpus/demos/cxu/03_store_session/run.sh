#!/usr/bin/env bash
# D3 verification: builds main.sla, runs it (SQLite via the node plugin's
# sqlite facade + sa_std JSON), then independently inspects the database
# file with python3's sqlite3 module.
set -u
cd "$(dirname "$0")"
export PATH=/content/sa_all/sci/zig-out/bin:$PATH

mkdir -p work
rm -f work/sessions.db

SA_PLUGIN_DEV=1 timeout 300 sa build-exe main.sla || exit 1
timeout 60 ./main || { echo "FAIL: demo exit code $?"; exit 1; }

python3 - << 'PYEOF'
import json, sqlite3, sys
con = sqlite3.connect("work/sessions.db")
rows = con.execute("SELECT id, turns, payload FROM sessions ORDER BY id").fetchall()
assert len(rows) == 2, rows
assert rows[0][0] == "s1" and rows[0][1] == 3, rows[0]
payload = json.loads(rows[0][2])
assert payload["model"] == "gpt-cxu" and abs(payload["temp"] - 0.2) < 1e-9, payload
assert rows[1][0] == "s2" and rows[1][1] == 2, rows[1]
print("python sqlite check ok")
PYEOF
[ $? -eq 0 ] || { echo "FAIL: python sqlite check"; exit 1; }
echo "d3 store+json ok"
