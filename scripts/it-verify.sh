#!/usr/bin/env bash
# Regtest integration test for frosted-verify (change add-verifier, M10).
# Fills a real chain with a market, a Persona and tagged bets (scripts/it-persona-thaw.sh), serves
# that public.db through frosted-api, then checks that the verifier reads the same truth from its own
# node: PASS on honest data, FAIL with exit code 1 once the API's copy is manipulated.
# The resolution and plan are pushed by it_thaw, so only market, pools and thaw are asserted here;
# plan and resolution recomputation against a real coordinator run is exercised by the unit tests
# and by `frosted-verify all` on an M8 market.
# Needs: scripts/dev-up.sh stack and a funded faucet (scripts/regtest-wallet.sh setup).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/.regtest/it-verify"
SRC="$ROOT/.regtest/it-persona-thaw"
NODE="${ZEBRA_RPC_URL:-http://127.0.0.1:18232}"
PORT=18081
API="http://127.0.0.1:$PORT/v1"
# Any valid, non-weak Ed25519 public key: this test never pushes to the ingest endpoints.
PUBKEY="a5dc32ed74bca7ded0a231ee810e97fa57ba3ca428439e92af2efd4094ad3761"

"$ROOT/scripts/it-persona-thaw.sh" >/dev/null
rm -rf "$STATE" && mkdir -p "$STATE"
cp "$SRC/public.db" "$STATE/public.db"
K_ROOT="$(cat "$SRC/k_root.txt")"
MID="$(python3 -I -c 'import json,sys;print(json.load(open(sys.argv[1]))["market_id"])' "$SRC/market.json")"
PERSONA_ID="$(sqlite3 "$STATE/public.db" "SELECT persona_id FROM persona LIMIT 1")"

cargo build -q -p frosted-api -p frosted-verify
V="$ROOT/target/debug/frosted-verify"
COORDINATOR_PUBKEY="$PUBKEY" API_BIND="127.0.0.1:$PORT" PUBLIC_DB_PATH="$STATE/public.db" \
  ZEBRA_RPC_URL="$NODE" LOG_LEVEL=warn "$ROOT/target/debug/frosted-api" >"$STATE/api.log" 2>&1 &
API_PID=$!
trap 'kill $API_PID 2>/dev/null || true' EXIT
for _ in $(seq 1 30); do curl -sf "$API/health" >/dev/null 2>&1 && break; sleep 1; done
curl -sf "$API/health" >/dev/null || { echo "FAIL frosted-api did not start" >&2; cat "$STATE/api.log" >&2; exit 1; }

v() { "$V" "$@" --node "$NODE" --api "$API"; }
pass() { echo "PASS $1"; }
fail() { echo "FAIL $1" >&2; exit 1; }

v market "$MID" --json > "$STATE/market.json" || fail "market"
python3 -I - "$STATE/market.json" <<'PY' || exit 1
import json, sys
r = json.load(open(sys.argv[1])); s = {c["id"]: c["status"] for c in r["checks"]}
assert r["ok"] and s["rules_hash"] == s["addresses"] == s["announcement"] == "PASS", s
PY
pass "market: rules_hash, Outcome Addresses and the on-chain announcement all PASS"

v pools "$MID" --json > "$STATE/pools.json" || fail "pools"
python3 -I - "$STATE/pools.json" <<'PY' || exit 1
import json, sys
r = json.load(open(sys.argv[1])); c = [c for c in r["checks"] if c["id"] == "pools"][0]
assert r["ok"] and c["status"] == "PASS", c
PY
pass "pools: identical to the API at the same height"

v thaw "$K_ROOT" --json > "$STATE/thaw.json" || fail "thaw"
curl -sf "$API/personas/$PERSONA_ID/report" > "$STATE/api-report.json"
python3 -I - "$STATE/thaw.json" "$STATE/api-report.json" <<'PY' || exit 1
import json, sys
mine = json.load(open(sys.argv[1])); api = json.load(open(sys.argv[2]))
assert mine["checks"]["ok"], mine["checks"]
assert mine["thaw_report"] == api, (mine["thaw_report"], api)
assert len(api["bets"]) == 4, api
PY
pass "thaw: the verifier's report is identical to the API's"

# The server-side check (S-10) must show the same verdict per check as the CLI at the same height.
set +e
v all "$MID" --json > "$STATE/all.json"
set -e
curl -sf "$API/markets/$MID/verify" > "$STATE/server-verify.json" || fail "GET /verify"
python3 -I - "$STATE/all.json" "$STATE/server-verify.json" <<'PY' || exit 1
import json, sys
cli = json.load(open(sys.argv[1])); srv = json.load(open(sys.argv[2]))
a = {c["id"]: c["status"] for c in cli["checks"]}; b = {c["id"]: c["status"] for c in srv["checks"]}
assert a == b, (a, b)
assert srv["trust"].startswith("Server-side check, for convenience only"), srv["trust"]
PY
pass "GET /v1/markets/{id}/verify shows the same PASS/FAIL per check as the CLI"

# Manipulate the API's copy: one extra zat in a pool.
sqlite3 "$STATE/public.db" "UPDATE bet SET amount_zat = amount_zat + 1 WHERE market_id = '$MID' AND status = 'COUNTED' AND outcome_index = 1 AND tagged = 1;"
set +e
v pools "$MID" >"$STATE/pools-tampered.txt"; CODE=$?
set -e
[ "$CODE" = 1 ] && grep -q "\[FAIL\] Pools per outcome" "$STATE/pools-tampered.txt" || fail "tampered pools were not detected (exit $CODE)"
pass "pools: a 1 zat manipulation FAILs with a diff and exit code 1"

echo "==> all verifier integration checks passed"
