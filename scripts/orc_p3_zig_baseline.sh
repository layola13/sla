#!/bin/bash
# ORC phase-3 stage A: Zig-side baseline timing under the ratified
# same-caliber protocol (docs/phase3/oracle-timing.md).
# RUN ON THE VENUE ONLY (Kaggle venue 3, hop 2202). Never on the local VM.
#
# Per probe (docs/probes/e2e_{a,b,c}.sla), end-to-end wall clock of
# `sa sla build` (fresh process every round, integer ms via date +%s%N,
# same method as scripts/bench.sh):
#   warm: 9 consecutive rounds with SLA_NO_FRESHNESS_CACHE=1. Venue
#         probe (2026-11-07, ORC line) proved a plain repeat build in
#         the same dir prints "freshness gate: hit, front-end skipped"
#         and does NOT run the frontend; with the freshness cache off
#         the frontend executes fully while the OS page cache, the
#         loaded plugin image and $HOME/.cache/sa/frontend stay warm.
#         Warm therefore = freshness gate off, everything else warm.
#   repeat_hit: 5 plain consecutive rounds (freshness gate on): the
#         short-circuit series, recorded under its own name, never to
#         be quoted as compile time.
#   cold: 9 rounds, before EVERY round the work dir's .sla-cache and
#         $HOME/.cache/sa/frontend are deleted (the Zig side's cache
#         surfaces, per protocol). Cold runs go last per probe because
#         the frontend cache is shared per-user on the venue.
# Plus per probe: one SLA_PROFILE=1 `sa sla check` and one
# SLA_PROFILE=1 `sa sla build` full-output capture (stage split), and
# a host-only micro-control (`sa --version` x5, no plugin command).
# Raw per-round ms land in results/p3/zig-baseline/<name>/{warm,cold}_ms.txt
# (one "ms rc" pair per line); no aggregation happens on the venue.
# Usage: bash scripts/orc_p3_zig_baseline.sh
set -u
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
source /content/tools/env.sh
export SA_STD_DIR=/content/sa_all/sci/sa_std
OUT=results/p3/zig-baseline
rm -rf "$OUT"
mkdir -p "$OUT/work"
{
  echo "== provenance =="
  date -u +"utc %Y-%m-%dT%H:%M:%SZ"
  hostname
  echo "sa: $(sa --version 2>&1)"
  echo "sa bin: $(command -v sa)"
  sha256sum "$(command -v sa)"
  echo "sci head: $(git -C /content/sa_all/sci rev-parse HEAD)"
  echo "sla head: $(git -C /content/sa_all/sa_plugin_sla rev-parse HEAD)"
  echo "installed plugin: /root/.local/share/sa_plugins/installed/sla/current/libsla.so"
  sha256sum /root/.local/share/sa_plugins/installed/sla/current/libsla.so
  stat -c 'size %s mtime %y' /root/.local/share/sa_plugins/installed/sla/current/libsla.so
  sha256sum /content/sa_all/sa_plugin_sla/zig-out/lib/libsla.so
  echo "SA_PLUGIN_DEV=$SA_PLUGIN_DEV SA_STD_DIR=$SA_STD_DIR"
  echo "nproc: $(nproc)"
  head -1 /proc/cpuinfo | true
  grep -m1 'model name' /proc/cpuinfo
  grep -m1 MemTotal /proc/meminfo
} > "$OUT/env.txt" 2>&1

time_ms() { # $1..: command; echoes "<ms> <rc>"
  local s e rc
  s=$(date +%s%N)
  "$@" >/dev/null 2>&1
  rc=$?
  e=$(date +%s%N)
  echo "$(( (e - s) / 1000000 )) $rc"
}

for p in docs/probes/e2e_a.sla docs/probes/e2e_b.sla docs/probes/e2e_c.sla; do
  n=$(basename "$p" .sla)
  d="$OUT/$n"
  w="$OUT/work/$n"
  mkdir -p "$d" "$w"
  cp "$p" "$w/"
  sha256sum "$p" > "$d/input_sha256.txt"

  # host-only micro-control: process start + host, no plugin command
  : > "$d/host_version_ms.txt"
  for i in 1 2 3 4 5; do
    time_ms sa --version >> "$d/host_version_ms.txt"
  done

  # warm rounds (freshness gate off; all other caches warm)
  : > "$d/warm_ms.txt"
  for i in $(seq 1 9); do
    rm -f "$w/$n.sa"
    ( cd "$w" && SLA_NO_FRESHNESS_CACHE=1 time_ms sa sla build "$n.sla" ) >> "$d/warm_ms.txt"
  done
  sha256sum "$w/$n.sa" > "$d/warm_out_sha256.txt" 2>&1

  # repeat-hit rounds (freshness gate on: front-end skipped by design)
  : > "$d/repeat_hit_ms.txt"
  for i in $(seq 1 5); do
    rm -f "$w/$n.sa"
    ( cd "$w" && time_ms sa sla build "$n.sla" ) >> "$d/repeat_hit_ms.txt"
  done

  # stage split captures (warm state)
  ( cd "$w" && SLA_PROFILE=1 sa sla check "$n.sla" ) > "$d/profile_check.txt" 2>&1
  echo $? > "$d/profile_check.rc"
  rm -f "$w/$n.sa"
  ( cd "$w" && SLA_PROFILE=1 sa sla build "$n.sla" ) > "$d/profile_build.txt" 2>&1
  echo $? > "$d/profile_build.rc"

  # cold rounds (per-round cache purge, per protocol)
  : > "$d/cold_ms.txt"
  : > "$d/cold_out_sha256.txt"
  for i in $(seq 1 9); do
    rm -rf "$w/.sla-cache" "$HOME/.cache/sa/frontend"
    rm -f "$w/$n.sa"
    ( cd "$w" && time_ms sa sla build "$n.sla" ) >> "$d/cold_ms.txt"
    sha256sum "$w/$n.sa" >> "$d/cold_out_sha256.txt" 2>&1
  done
done
echo DONE > "$OUT/FINISHED"
