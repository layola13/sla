#!/bin/bash
# Sync local master (source of truth) to the Kaggle venue.
# Usage: bash scripts/sync_to_venue.sh [port]   (default 2202)
# Venue history: 2201 = venue 2 (expired 2026-11-07); 2202 = venue 3
# (kernel icbnsk/muse-kaggle-venue-vpsdirect3, hold to ~19:43 CST).
#
# Three hops, each kept in the shape already proven on these venues:
# wipe the synced paths, extract over a clean tree, then verify against
# a local md5 manifest. (2026-11-07 incident: overlay extraction onto
# venue 3's pre-seeded older snapshot left 3 files stale/missing while
# the file count still matched — an unverified sync is not a sync. The
# venue's target/ dir is preserved across syncs.)
set -eu
PORT="${1:-2202}"
HOP="$HOME/workspace/kaggle-venue-tunnel/hop_vpsdirect.sh"
cd "$(dirname "$0")/.."
bash "$HOP" "$PORT" 'rm -rf /root/sla-plugin-rust/Cargo.toml /root/sla-plugin-rust/Cargo.lock /root/sla-plugin-rust/crates /root/sla-plugin-rust/docs /root/sla-plugin-rust/scripts /root/sla-plugin-rust/fixtures'
tar -czf - --exclude=target --exclude=.git --exclude=docs/build-logs . | \
  bash "$HOP" "$PORT" 'mkdir -p /root/sla-plugin-rust && tar xzf - -C /root/sla-plugin-rust && echo SYNC_OK'
find . -path ./target -prune -o -path ./.git -prune -o -type f -print \
  | grep -v build-logs | sort | xargs md5sum > /tmp/sync_manifest.md5
bash "$HOP" "$PORT" 'cat > /tmp/sync_manifest.md5' < /tmp/sync_manifest.md5
BAD=$(bash "$HOP" "$PORT" 'cd /root/sla-plugin-rust && md5sum -c /tmp/sync_manifest.md5 2>/dev/null | grep -cv ": OK"')
if [ "$BAD" != "0" ]; then
  echo "SYNC_VERIFY_FAILED: $BAD files differ on venue" >&2
  exit 1
fi
echo SYNC_OK_VERIFIED
