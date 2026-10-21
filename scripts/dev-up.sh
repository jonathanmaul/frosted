#!/usr/bin/env bash
# Starts the Frosted regtest stack (zebrad + Zaino) and mines past NU6.3 (Ironwood).
# Idempotent: reuses running containers and never resets chain state.
# Exit code 0 only if the tip is past NU6.3 and getblockchaininfo lists an `ironwood` pool.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
COMPOSE=(docker compose -f "$ROOT/infra/docker-compose.yml")
RPC_URL="${ZEBRA_RPC_URL:-http://127.0.0.1:18232}"
MIN_CONFIRMATIONS="${MIN_CONFIRMATIONS:-3}"

# Coinbase recipient: FROSTED_MINER_ADDRESS if set (scripts/regtest-wallet.sh uses it while
# funding the test wallet), otherwise the default address in infra/docker-compose.yml.

rpc() {
  curl -sf -H 'content-type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-[]}}" "$RPC_URL"
}

echo "==> starting regtest stack"
"${COMPOSE[@]}" up -d

echo "==> waiting for zebrad RPC at $RPC_URL"
for _ in $(seq 1 60); do
  rpc getblockcount >/dev/null 2>&1 && break
  sleep 2
done
rpc getblockcount >/dev/null || { echo "zebrad RPC did not come up" >&2; exit 1; }

info="$(rpc getblockchaininfo)"
read -r tip nu63 < <(python3 -I -c '
import json, sys
r = json.load(sys.stdin)["result"]
nu63 = next((u["activationheight"] for u in r["upgrades"].values() if u["name"] == "NU6.3"), "")
print(r["blocks"], nu63)' <<<"$info")

if [ -z "$nu63" ]; then
  echo "NU6.3 is not configured on this node; check infra/zebra/regtest.toml" >&2
  exit 1
fi

target=$((nu63 + MIN_CONFIRMATIONS))
if [ "$tip" -lt "$target" ]; then
  echo "==> mining $((target - tip)) blocks (tip $tip, NU6.3 at $nu63)"
  rpc generate "[$((target - tip))]" >/dev/null
fi

info="$(rpc getblockchaininfo)"
python3 -I -c '
import json, sys
r = json.load(sys.stdin)["result"]
pools = {p["id"]: p.get("chainValueZat") for p in r["valuePools"]}
nu63 = next(u for u in r["upgrades"].values() if u["name"] == "NU6.3")
print("tip=%s NU6.3 activation=%s status=%s" % (r["blocks"], nu63["activationheight"], nu63["status"]))
print("valuePools:", pools)
if "ironwood" not in pools or nu63["status"] != "active":
    sys.exit("ironwood value pool missing or NU6.3 not active")' <<<"$info"

# Zaino shares zebra's network namespace; if zebra was recreated, Zaino must be too.
zaino_up() { nc -z 127.0.0.1 9067 >/dev/null 2>&1; }
for attempt in 1 2; do
  for _ in $(seq 1 30); do zaino_up && break; sleep 2; done
  zaino_up && break
  if [ "$attempt" = 1 ]; then
    echo "==> Zaino gRPC unreachable, recreating zaino"
    "${COMPOSE[@]}" up -d --force-recreate zaino
  fi
done
zaino_up || { echo "Zaino gRPC did not come up on 127.0.0.1:9067" >&2; exit 1; }

echo "==> regtest stack ready (zebrad $RPC_URL, Zaino gRPC 127.0.0.1:9067)"
