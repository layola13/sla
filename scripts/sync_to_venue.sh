#!/bin/bash
# Sync local master (source of truth) to the Kaggle venue.
# Usage: bash scripts/sync_to_venue.sh [port]   (default 2201)
set -eu
PORT="${1:-2201}"
cd "$(dirname "$0")/.."
tar -czf - --exclude=target --exclude=.git . | \
  bash ~/workspace/kaggle-venue-tunnel/hop_vpsdirect.sh "$PORT" \
  'mkdir -p /root/sla-plugin-rust && tar xzf - -C /root/sla-plugin-rust && echo SYNC_OK'
