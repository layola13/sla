#!/bin/bash
# ORC phase-3 stage B: expanded-set (8 items) byte-parity regression +
# same-window dual-side timing under the ratified protocol
# (docs/phase3/oracle-timing.md sections 1.1-1.4; stage A numbers are
# historical only, both sides are re-measured in this window).
# RUN ON THE VENUE ONLY (Kaggle venue 3, hop 2202). Never on the local VM.
#
# Items: e2e_a/e2e_b/e2e_c (docs/probes) + fixtures 01/02/03/04/07.
# Regress: Rust sla-oracle run1+run2 (run2 with SLA_RS_PROFILE=1),
# Zig `sa sla build` in two fresh work dirs, sha256 of everything,
# diffs rust-vs-zig / run-vs-run / zig-vs-zig / both-vs-expected.
# Timing: 9 rounds, interleaved (per round, per item, the four series
# run in alternating side order). Series per item:
#   zig_warm   SLA_NO_FRESHNESS_CACHE=1, all other caches warm
#   zig_cold   per-round rm of workdir .sla-cache + ~/.cache/sa/frontend
#   rust_warm  fresh sla-oracle process, consecutive
#   rust_cold  fresh sla-oracle process (oracle has NO disk cache;
#              cold == warm by construction, recorded separately anyway)
#   zig_repeat_hit  5 plain rounds (freshness gate on; reference only,
#              never a compile time)
# Raw per-round "ms rc" lines land in results/p3/phase-b/timing/<item>/.
# No aggregation on the venue.
# Usage: bash scripts/orc_p3_phaseb.sh
set -u
export PATH="$HOME/.cargo/bin:$PATH"
cd "$(dirname "$0")/.."
source /content/tools/env.sh
export SA_STD_DIR=/content/sa_all/sci/sa_std
OUT=results/p3/phase-b
rm -rf "$OUT"
mkdir -p "$OUT/regress" "$OUT/timing" "$OUT/work/zig" "$OUT/work/rust"

ITEMS="e2e_a e2e_b e2e_c 01_hello 02_arith 03_if_else 04_while_sum 07_functions"
src_of() {
  case "$1" in
    e2e_*) echo "docs/probes/$1.sla" ;;
    *) echo "fixtures/$1.sla" ;;
  esac
}

{
  echo "== provenance =="
  date -u +"utc %Y-%m-%dT%H:%M:%SZ"
  hostname
  echo "sa: $(sa --version 2>&1)"
  echo "sa bin: $(command -v sa)"
  sha256sum "$(command -v sa)"
  echo "sci head: $(git -C /content/sa_all/sci rev-parse HEAD)"
  echo "sla head: $(git -C /content/sa_all/sa_plugin_sla rev-parse HEAD)"
  echo "rustc: $(rustc --version)"
  echo "installed plugin: /root/.local/share/sa_plugins/installed/sla/current/libsla.so"
  sha256sum /root/.local/share/sa_plugins/installed/sla/current/libsla.so
  stat -c 'size %s mtime %y' /root/.local/share/sa_plugins/installed/sla/current/libsla.so
  sha256sum /content/sa_all/sa_plugin_sla/zig-out/lib/libsla.so
  echo "SA_PLUGIN_DEV=$SA_PLUGIN_DEV SA_STD_DIR=$SA_STD_DIR"
  echo "nproc: $(nproc)"
  grep -m1 'model name' /proc/cpuinfo
  grep -m1 MemTotal /proc/meminfo
  uptime
  echo "== corpus source sha256 =="
  for n in $ITEMS; do sha256sum "$(src_of "$n")"; done
  echo "== fixtures expected sha256 =="
  sha256sum fixtures/expected/*.sa
} > "$OUT/env.txt" 2>&1

# Freshness guard (cargo stale-binary lesson): touch every Rust source
# so the timed/compared binary is provably built from the current tree,
# then record the binary sha + a self-proof run (e2e_c gold sha).
find crates -name '*.rs' -exec touch {} +
cargo build --release > "$OUT/cargo_build.log" 2>&1
echo $? > "$OUT/cargo_build.rc"
BIN=target/release/sla-oracle
{
  sha256sum "$BIN"
  stat -c 'size %s mtime %y' "$BIN"
  "$BIN" docs/probes/e2e_c.sla -o "$OUT/selfproof_e2e_c.sa" 2> "$OUT/selfproof_e2e_c.stderr"
  echo "selfproof rc $?"
  sha256sum "$OUT/selfproof_e2e_c.sa"
  echo "expected e2e_c gold fe1902818c4a175e630cbe0d362f215693b0aeb7efbd6efdf7a5109d59b47ada"
} > "$OUT/oracle_bin.txt" 2>&1

time_ms() { # $1..: command; echoes "<ms> <rc>"
  local s e rc
  s=$(date +%s%N)
  "$@" >/dev/null 2>&1
  rc=$?
  e=$(date +%s%N)
  echo "$(( (e - s) / 1000000 )) $rc"
}

# ---------------- regress ----------------
for n in $ITEMS; do
  src=$(src_of "$n")
  d="$OUT/regress/$n"
  mkdir -p "$d/dump1" "$d/dump2" "$d/zigwork1" "$d/zigwork2"
  "$BIN" "$src" -o "$d/rust1.sa" --dump-boundaries "$d/dump1" > "$d/rust1.stdout" 2> "$d/rust1.stderr"
  echo $? > "$d/rust1.rc"
  SLA_RS_PROFILE=1 "$BIN" "$src" -o "$d/rust2.sa" --dump-boundaries "$d/dump2" > "$d/rust2.stdout" 2> "$d/rust2.stderr"
  echo $? > "$d/rust2.rc"
  cp "$src" "$d/zigwork1/"
  ( cd "$d/zigwork1" && sa sla build "$n.sla" > stdout.txt 2> stderr.txt; echo $? > rc; ls -la > ls.txt 2>&1 )
  cp "$src" "$d/zigwork2/"
  ( cd "$d/zigwork2" && sa sla build "$n.sla" > stdout.txt 2> stderr.txt; echo $? > rc; ls -la > ls.txt 2>&1 )
  {
    sha256sum "$src" "$d/rust1.sa" "$d/rust2.sa" "$d/zigwork1/$n.sa" "$d/zigwork2/$n.sa"
    case "$n" in e2e_*) ;; *) sha256sum "fixtures/expected/$n.sa" ;; esac
  } > "$d/sha256.txt" 2>&1
  diff "$d/rust1.sa" "$d/zigwork1/$n.sa" > "$d/diff_rust_vs_zig.txt" 2>&1; echo $? > "$d/diff_rust_vs_zig.rc"
  diff "$d/rust1.sa" "$d/rust2.sa" > "$d/diff_rust_run1_run2.txt" 2>&1; echo $? > "$d/diff_rust_run1_run2.rc"
  diff "$d/zigwork1/$n.sa" "$d/zigwork2/$n.sa" > "$d/diff_zig_zig2.txt" 2>&1; echo $? > "$d/diff_zig_zig2.rc"
  case "$n" in
    e2e_*) ;;
    *)
      diff "$d/rust1.sa" "fixtures/expected/$n.sa" > "$d/diff_rust_vs_expected.txt" 2>&1; echo $? > "$d/diff_rust_vs_expected.rc"
      diff "$d/zigwork1/$n.sa" "fixtures/expected/$n.sa" > "$d/diff_zig_vs_expected.txt" 2>&1; echo $? > "$d/diff_zig_vs_expected.rc"
      ;;
  esac
done

# ---------------- timing ----------------
# process-start controls (global): Zig host alone vs oracle usage-exit.
: > "$OUT/timing/host_version_ms.txt"
: > "$OUT/timing/oracle_start_ms.txt"
for i in $(seq 1 9); do
  time_ms sa --version >> "$OUT/timing/host_version_ms.txt"
  time_ms "$BIN" >> "$OUT/timing/oracle_start_ms.txt"
done

for n in $ITEMS; do
  src=$(src_of "$n")
  zw="$OUT/work/zig/$n"; rw="$OUT/work/rust/$n"
  mkdir -p "$OUT/timing/$n" "$zw" "$rw"
  cp "$src" "$zw/"
  cp "$src" "$rw/"
  sha256sum "$src" > "$OUT/timing/$n/input_sha256.txt"
  : > "$OUT/timing/$n/zig_warm_ms.txt"; : > "$OUT/timing/$n/zig_warm_out_sha256.txt"
  : > "$OUT/timing/$n/zig_cold_ms.txt"; : > "$OUT/timing/$n/zig_cold_out_sha256.txt"
  : > "$OUT/timing/$n/rust_warm_ms.txt"; : > "$OUT/timing/$n/rust_warm_out_sha256.txt"
  : > "$OUT/timing/$n/rust_cold_ms.txt"; : > "$OUT/timing/$n/rust_cold_out_sha256.txt"
done

zig_warm_run() { # $1=item
  local n=$1 zw="$OUT/work/zig/$1"
  rm -f "$zw/$n.sa"
  ( cd "$zw" && SLA_NO_FRESHNESS_CACHE=1 time_ms sa sla build "$n.sla" ) >> "$OUT/timing/$n/zig_warm_ms.txt"
  sha256sum "$zw/$n.sa" >> "$OUT/timing/$n/zig_warm_out_sha256.txt" 2>&1
}
zig_cold_run() { # $1=item
  local n=$1 zw="$OUT/work/zig/$1"
  rm -rf "$zw/.sla-cache" "$HOME/.cache/sa/frontend"
  rm -f "$zw/$n.sa"
  ( cd "$zw" && time_ms sa sla build "$n.sla" ) >> "$OUT/timing/$n/zig_cold_ms.txt"
  sha256sum "$zw/$n.sa" >> "$OUT/timing/$n/zig_cold_out_sha256.txt" 2>&1
}
rust_warm_run() { # $1=item
  local n=$1 rw="$OUT/work/rust/$1"
  rm -f "$rw/$n.sa"
  time_ms "$BIN" "$rw/$n.sla" -o "$rw/$n.sa" >> "$OUT/timing/$n/rust_warm_ms.txt"
  sha256sum "$rw/$n.sa" >> "$OUT/timing/$n/rust_warm_out_sha256.txt" 2>&1
}
rust_cold_run() { # $1=item; oracle has no disk cache: fresh process is the cold form
  local n=$1 rw="$OUT/work/rust/$1"
  rm -f "$rw/$n.sa"
  time_ms "$BIN" "$rw/$n.sla" -o "$rw/$n.sa" >> "$OUT/timing/$n/rust_cold_ms.txt"
  sha256sum "$rw/$n.sa" >> "$OUT/timing/$n/rust_cold_out_sha256.txt" 2>&1
}

# Interleaved rounds: alternate the starting side per round so neither
# side systematically inherits the other's load tail.
for r in $(seq 1 9); do
  for n in $ITEMS; do
    if [ $((r % 2)) -eq 1 ]; then
      zig_warm_run "$n"; rust_warm_run "$n"; zig_cold_run "$n"; rust_cold_run "$n"
    else
      rust_warm_run "$n"; zig_warm_run "$n"; rust_cold_run "$n"; zig_cold_run "$n"
    fi
  done
done

# repeat-hit reference series (freshness gate on; never a compile time)
for n in $ITEMS; do
  zw="$OUT/work/zig/$n"
  : > "$OUT/timing/$n/zig_repeat_hit_ms.txt"
  for i in $(seq 1 5); do
    rm -f "$zw/$n.sa"
    ( cd "$zw" && time_ms sa sla build "$n.sla" ) >> "$OUT/timing/$n/zig_repeat_hit_ms.txt"
  done
  # stage-split captures in warm state
  ( cd "$zw" && SLA_PROFILE=1 sa sla check "$n.sla" ) > "$OUT/timing/$n/zig_profile_check.txt" 2>&1
  echo $? > "$OUT/timing/$n/zig_profile_check.rc"
  rm -f "$zw/$n.sa"
  ( cd "$zw" && SLA_PROFILE=1 sa sla build "$n.sla" ) > "$OUT/timing/$n/zig_profile_build.txt" 2>&1
  echo $? > "$OUT/timing/$n/zig_profile_build.rc"
  rw="$OUT/work/rust/$n"
  SLA_RS_PROFILE=1 "$BIN" "$rw/$n.sla" -o "$rw/$n.profile.sa" > "$OUT/timing/$n/rust_profile.stdout" 2> "$OUT/timing/$n/rust_profile.txt"
  echo $? > "$OUT/timing/$n/rust_profile.rc"
done

{ date -u +"utc %Y-%m-%dT%H:%M:%SZ"; uptime; } > "$OUT/finished_env.txt" 2>&1
echo DONE > "$OUT/FINISHED"
