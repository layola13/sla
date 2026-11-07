#!/usr/bin/env bash
# D4 verification: builds main.sla, runs it with the CXU_MODEL env
# override set (TOML values come from sa_std's built-in sa_toml_*), then
# independently parses the same config.toml with python3's tomllib and
# checks the same values.
set -u
cd "$(dirname "$0")"
export PATH=/content/sa_all/sci/zig-out/bin:$PATH

SA_PLUGIN_DEV=1 timeout 300 sa build-exe main.sla || exit 1
CXU_MODEL=gpt-env-override timeout 60 ./main || { echo "FAIL: demo exit code $?"; exit 1; }

python3 - << 'PYEOF'
import tomllib
with open("config.toml", "rb") as f:
    cfg = tomllib.load(f)
assert cfg["model"] == "gpt-cxu", cfg["model"]
assert abs(cfg["temperature"] - 0.2) < 1e-9
assert cfg["max_turns"] == 42 and cfg["debug"] is False
prov = cfg["model_providers"]["cxu"]
assert prov["base_url"] == "https://example.invalid/v1" and prov["requires_auth"] is True
jex = cfg["mcp_servers"]["jex"]
assert jex["args"] == ["server.js", "--port", "8080"], jex["args"]
assert jex["env"]["RETRIES"] == 3
assert [s["port"] for s in cfg["servers"]] == [8001, 8002]
print("python tomllib check ok")
PYEOF
[ $? -eq 0 ] || { echo "FAIL: python tomllib check"; exit 1; }
echo "d4 config+env ok"
