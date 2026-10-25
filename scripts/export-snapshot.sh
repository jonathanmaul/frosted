#!/usr/bin/env bash
# Records the public API responses of settled markets into web/lib/snapshot.json, for a Vercel deployment
# without a live backend (NEXT_PUBLIC_SNAPSHOT_MODE=true, spec M12.5). Public endpoints only.
#   scripts/export-snapshot.sh <market_id> [market_id...]
# Needs a running frosted-api (e.g. scripts/testnet.sh up); API defaults to the testnet stack's port.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API="${API:-http://127.0.0.1:28080/v1}"
[ $# -ge 1 ] || { echo "usage: scripts/export-snapshot.sh <market_id> [market_id...]" >&2; exit 2; }
curl -sf "$API/health" >/dev/null || { echo "error: no API at $API" >&2; exit 1; }
cd "$ROOT/web" && node scripts/snapshot.mjs "$API" "$@"
