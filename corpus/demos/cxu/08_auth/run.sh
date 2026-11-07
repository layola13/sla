#!/usr/bin/env bash
# D8 auth demo: builds main.sla and runs it inside a throwaway D-Bus
# session with gnome-keyring unlocked (empty password), so the OS
# keyring half exercises a real Secret Service. All assertions live
# inside the program (distinct exit codes per check); success prints
# "d8 auth ok".
set -u
cd "$(dirname "$0")"
export PATH=/content/sa_all/sci/zig-out/bin:/content/sa_all/sa_plugin_sla/zig-out/bin:$PATH
export SA_PLUGIN_DEV=1
rm -f main main.sa main.sa.bc main.test main.test.sa
timeout 300 sa build-exe main.sla || { echo "D8 BUILD FAILED"; exit 1; }
timeout 120 dbus-run-session -- bash -c '
  eval "$(printf "" | gnome-keyring-daemon --unlock --components=secrets 2>/dev/null)"
  exec ./main
'
rc=$?
if [ $rc -ne 0 ]; then echo "D8 FAILED rc=$rc"; exit 1; fi
echo "d8 auth demo ok"
