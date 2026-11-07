#!/usr/bin/env bash
# D6 fs watch demo: builds main.sla and runs it against a scratch work
# directory. All assertions live inside the program (distinct exit
# codes per check); success prints "d6 fs watch ok".
set -u
cd "$(dirname "$0")"
export PATH=/content/sa_all/sci/zig-out/bin:/content/sa_all/sa_plugin_sla/zig-out/bin:$PATH
export SA_PLUGIN_DEV=1
rm -f main main.sa main.sa.bc main.test main.test.sa
rm -rf work
mkdir -p work
timeout 300 sa build-exe main.sla || { echo "D6 BUILD FAILED"; exit 1; }
timeout 60 ./main
rc=$?
if [ $rc -ne 0 ]; then echo "D6 FAILED rc=$rc"; exit 1; fi
echo "d6 fs watch demo ok"
