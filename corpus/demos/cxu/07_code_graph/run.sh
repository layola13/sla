#!/usr/bin/env bash
# D7 code graph demo: builds main.sla and runs it. The tree-sitter
# core + TypeScript grammar are the sa_std artifacts built by
# sci/artifacts/tree_sitter/build_tree_sitter.sh. All assertions live
# inside the program (distinct exit codes per check); success prints
# "d7 code graph ok".
set -u
cd "$(dirname "$0")"
export PATH=/content/sa_all/sci/zig-out/bin:/content/sa_all/sa_plugin_sla/zig-out/bin:$PATH
export SA_PLUGIN_DEV=1
export SA_TREE_SITTER_DIR=/content/sa_all/sci/artifacts/tree_sitter
rm -f main main.sa main.sa.bc main.test main.test.sa
timeout 300 sa build-exe main.sla || { echo "D7 BUILD FAILED"; exit 1; }
timeout 60 ./main
rc=$?
if [ $rc -ne 0 ]; then echo "D7 FAILED rc=$rc"; exit 1; fi
echo "d7 code graph demo ok"
