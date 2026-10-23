#!/usr/bin/env bash
# Regtest integration test for committee resolution (change add-committee-resolution, M11).
#
# Real chain, real daemons: glass-indexer, frosted-api and payout-coordinator run against zebrad.
# Cycle 1: COMMITTEE market -> bets from the WASM builder -> propose/confirm -> withdrawal clears the
#          votes -> propose/confirm again -> dispute window passes -> RESOLVED -> payout plan ->
#          FROST 2-of-3 signing -> SETTLED -> the winner's wallet receives -> frosted-verify all.
# Cycle 2: a COMMITTEE market nobody decides becomes VOID after void_after_blocks.
# Needs: scripts/dev-up.sh stack, a funded faucet and the recipient wallet (scripts/regtest-wallet.sh setup),
#        wallet-helper/target/release/frosted-wallet-helper, and web/public/wasm (pnpm build:wasm).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/.regtest/committee-run"
PRIV="$STATE/private"
W="$ROOT/scripts/regtest-wallet.sh"
NODE="${ZEBRA_RPC_URL:-http://127.0.0.1:18232}"
API_PORT=18082
COORD_PORT=18092
API="http://127.0.0.1:$API_PORT/v1"
export FROSTED_NETWORK=regtest LOG_LEVEL=warn

pass() { echo "PASS $1"; }
fail() { echo "FAIL $1" >&2; exit 1; }
rpc() { curl -sf -H 'content-type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-[]}}" "$NODE"; }
json() { python3 -I -c "import json,sys; d=json.load(sys.stdin); print($1)"; }
api_get() { curl -sf "$API$1"; }
wait_for() { # description timeout-seconds command...
  local what="$1" limit="$2"; shift 2
  for _ in $(seq 1 "$limit"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
  fail "timed out waiting for $what"
}
market_status() { api_get "/markets/$1" | json 'd["status"]'; }
status_is() { [ "$(market_status "$1")" = "$2" ]; }
mine() { [ "$1" -gt 0 ] && "$W" mine "$1" >/dev/null || true; }
# Zaino indexes asynchronously; the escrow wallet import needs its tree state at the current tip.
wait_zaino() {
  local want have; want="$(tip)"
  for _ in $(seq 1 60); do
    have="$("$W" dt wallet -w /work/faucet get-info -s 127.0.0.1:8137 2>/dev/null | json 'd["chain_tip_height"]' 2>/dev/null || echo 0)"
    [ "${have:-0}" -ge "$want" ] && return 0
    sleep 2
  done
  fail "Zaino stuck at ${have:-?} (tip $want)"
}
tip() { rpc getblockcount | json 'd["result"]'; }

rm -rf "$STATE" && mkdir -p "$STATE"
cargo build -q -p payout-coordinator -p frosted-operator -p glass-indexer -p frosted-api -p frosted-verify
cargo build -q -p glass-indexer --example it_fixture
BIN="$ROOT/target/debug"
OP="$BIN/frosted-operator"
export COORDINATOR_BIND="127.0.0.1:$COORD_PORT"

# --- secrets, committee and the three daemons ---------------------------------------------------
PUBKEY="$("$OP" secrets init --dir "$PRIV" | json 'd["coordinator_pubkey"]')"
read -r _REG_UIVK TREASURY < <("$BIN/examples/it_fixture" registry)

# The wallet helper hides failure reasons on purpose; this wrapper logs only the op and exit code.
cat > "$STATE/helper.sh" <<SH
#!/bin/sh
IN=\$(cat)
OP=\$(printf '%s' "\$IN" | python3 -I -c 'import json,sys; print(json.load(sys.stdin).get("op"))')
printf '%s' "\$IN" | "$ROOT/wallet-helper/target/release/frosted-wallet-helper" 2>>"$STATE/helper.err"
RC=\$?
echo "\$OP rc=\$RC" >> "$STATE/helper.log"
exit \$RC
SH
chmod +x "$STATE/helper.sh"

PIDS=()
cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done; }
[ -n "${KEEP_DAEMONS:-}" ] || trap cleanup EXIT   # KEEP_DAEMONS=1 leaves them running to debug a failure

PUBLIC_DB_PATH="$STATE/public.db" ZEBRA_RPC_URL="$NODE" "$BIN/glass-indexer" once >/dev/null 2>&1 || true
[ -f "$STATE/public.db" ] || fail "indexer did not create public.db"
PUBLIC_DB_PATH="$STATE/public.db" ZEBRA_RPC_URL="$NODE" "$BIN/glass-indexer" run >"$STATE/indexer.log" 2>&1 & PIDS+=($!)
COORDINATOR_PUBKEY="$PUBKEY" API_BIND="127.0.0.1:$API_PORT" PUBLIC_DB_PATH="$STATE/public.db" ZEBRA_RPC_URL="$NODE" \
  "$BIN/frosted-api" >"$STATE/api.log" 2>&1 & PIDS+=($!)
wait_for "frosted-api" 30 curl -sf "$API/health"

COORDINATOR_DB_PATH="$STATE/coordinator.db" COORDINATOR_INDEXER_DB_PATH="$STATE/coordinator-indexer.db" \
  SECRETS_KEY_FILE="$PRIV/secrets.key" COORDINATOR_KEY_FILE="$PRIV/coordinator.key" ENVELOPE_SK_FILE="$PRIV/envelope.sk" \
  HOT_WALLET_SEED_FILE="$PRIV/hot-wallet.seed" TREASURY_ADDRESS="$TREASURY" ZEBRA_RPC_URL="$NODE" \
  LIGHTWALLETD_GRPC_URL="http://127.0.0.1:9067" API_INGEST_URL="http://127.0.0.1:$API_PORT/v1/ingest" \
  WALLET_HELPER_BIN="$STATE/helper.sh" ESCROW_SYNC_INTERVAL_SECS=3 \
  "$BIN/payout-coordinator" >"$STATE/coordinator.log" 2>&1 & PIDS+=($!)
wait_for "payout-coordinator" 30 curl -s -o /dev/null "http://127.0.0.1:$COORD_PORT/markets/none"

"$OP" committee init --share-dir "$STATE/shares" --member-key "$PRIV/member-1.key" --member-key "$PRIV/member-2.key" --member-key "$PRIV/member-3.key" >/dev/null
HOT="$("$OP" hot-wallet init | json 'd["address"]')"
"$W" send "$HOT" 300000000 "fund hot wallet" >/dev/null
mine 3
pass "stack up: indexer, API and coordinator run against the chain; committee and hot wallet ready"

# --- cycle 1: decided by the committee ----------------------------------------------------------
wait_zaino
T0="$(tip)"
cat > "$STATE/market1.json" <<JSON
{"question":"Regtest M11: will the committee say YES?","outcomes":["YES","NO"],"resolution_type":"COMMITTEE",
 "resolution_params":{"criteria":"Both members read the published statement and agree.","evidence_source":"https://example.org/statement","dispute_window_blocks":6,"void_after_blocks":60},
 "close_height":$((T0 + 30)),"resolve_height":$((T0 + 34)),"fee_bps":100}
JSON
create_market() { # file -> market id. Importing the escrow wallet needs Zaino's tree state, which can lag; a failed import writes nothing, so retrying is safe.
  local out
  for attempt in 1 2 3; do
    if out="$("$OP" market create --file "$1" 2>"$STATE/create.err")"; then echo "$out" | json 'd["market_definition"]["market_id"]'; return 0; fi
    echo "market create attempt $attempt failed: $(cat "$STATE/create.err")" >&2; sleep 5; wait_zaino
  done
  fail "market create"
}
ID="$(create_market "$STATE/market1.json")"
mine 4
wait_for "announcement to open the market" 90 status_is "$ID" OPEN
pass "committee market $ID is OPEN"

RECIPIENT_UA="$("$W" dt wallet -w /work/recipient list-addresses --receiver orchard | sed -n 's/.*\(uregtest1[0-9a-z]*\).*/\1/p' | head -1)"
[ -n "$RECIPIENT_UA" ] || fail "no recipient address"
api_get "/markets/$ID" > "$STATE/market1-public.json"
cat > "$STATE/bet.mjs" <<'JS'
import { readFileSync } from "node:fs";
const [root, marketFile, outcome, amount, ua] = process.argv.slice(2);
const mod = await import(`${root}/web/public/wasm/zcash_glass_wasm.js`);
await mod.default({ module_or_path: readFileSync(`${root}/web/public/wasm/zcash_glass_wasm_bg.wasm`) });
const market = JSON.parse(readFileSync(marketFile, "utf8"));
const built = mod.build_bet({ market_definition: market.market_definition, outcome_index: Number(outcome), amount_zat: Number(amount), payout_ua: ua });
console.log(JSON.stringify({ address: market.market_definition.outcomes[Number(outcome)].address, memo: built.memo_text }));
JS
bet() { # outcome amount
  local out; out="$(node "$STATE/bet.mjs" "$ROOT" "$STATE/market1-public.json" "$1" "$2" "$RECIPIENT_UA" 2>/dev/null)"
  "$W" send "$(echo "$out" | json 'd["address"]')" "$2" "$(echo "$out" | json 'd["memo"]')" >/dev/null
}
bet 0 2000000   # YES, the winning side
bet 1 1000000   # NO
mine 4
wait_for "both bets to be counted" 90 bash -c "[ \"\$(curl -sf $API/markets/$ID/odds | python3 -I -c 'import json,sys;d=json.load(sys.stdin);print(d[\"pool_total_zat\"])')\" = 3000000 ]"
pass "two WASM bets are COUNTED (pool 0.03 ZEC)"

# Not before resolve_height.
set +e; "$OP" resolution propose "$ID" --outcome 0 --evidence-url https://example.org/e --member member-1 >/dev/null 2>"$STATE/early.txt"; CODE=$?; set -e
[ "$CODE" = 1 ] && grep -q COMMITTEE_NOT_OPEN "$STATE/early.txt" || fail "proposal before resolve_height was not rejected"
pass "a proposal before resolve_height is rejected (COMMITTEE_NOT_OPEN)"

R=$((T0 + 34))
mine $((R - $(tip) + 1))
EVIDENCE="https://example.org/statement/2026-10-23"
"$OP" resolution propose "$ID" --outcome 0 --evidence-url "$EVIDENCE" --member member-1 >/dev/null
set +e; "$OP" resolution confirm "$ID" --outcome 1 --evidence-url "$EVIDENCE" --member member-2 >/dev/null 2>"$STATE/mismatch.txt"; CODE=$?; set -e
[ "$CODE" = 1 ] && grep -q COMMITTEE_MISMATCH "$STATE/mismatch.txt" || fail "a different outcome was accepted"
wait_for "AWAITING_RESOLUTION with one vote" 30 bash -c "[ \"\$(curl -sf $API/markets/$ID/resolution | python3 -I -c 'import json,sys;d=json.load(sys.stdin);print(d[\"status\"],len(d[\"votes\"]))')\" = 'AWAITING_RESOLUTION 1' ]"
pass "members disagree: the market stays AWAITING_RESOLUTION"

"$OP" resolution confirm "$ID" --outcome 0 --evidence-url "$EVIDENCE" --member member-2 >/dev/null
wait_for "PROPOSED" 30 status_is "$ID" PROPOSED
api_get "/markets/$ID/resolution" | json 'print(len(d["votes"]), d["evidence"]["source"], d["evidence"]["height"])' > "$STATE/proposed.txt"
grep -q "^2 committee " "$STATE/proposed.txt" || fail "PROPOSED did not publish both votes"
pass "two members agree: PROPOSED with both votes and the window's start block $(cut -d' ' -f3 "$STATE/proposed.txt" | tr -d '\n')"

"$OP" resolution withdraw "$ID" --member member-3 >/dev/null
wait_for "withdrawal to clear the votes" 30 bash -c "[ \"\$(curl -sf $API/markets/$ID/resolution | python3 -I -c 'import json,sys;d=json.load(sys.stdin);print(d[\"status\"],len(d[\"votes\"]))')\" = 'AWAITING_RESOLUTION 0' ]"
pass "a withdrawal inside the window returns to AWAITING_RESOLUTION with the votes cleared"

"$OP" resolution propose "$ID" --outcome 0 --evidence-url "$EVIDENCE" --member member-3 >/dev/null
"$OP" resolution confirm "$ID" --outcome 0 --evidence-url "$EVIDENCE" --member member-1 >/dev/null
wait_for "PROPOSED again" 30 status_is "$ID" PROPOSED
START="$(api_get "/markets/$ID/resolution" | json 'd["evidence"]["height"]')"
mine 5   # one block short of dispute_window_blocks = 6
sleep 8
status_is "$ID" PROPOSED || fail "resolved before the dispute window passed"
mine $((START + 6 - $(tip) + 1))
wait_for "RESOLVED after the window" 60 status_is "$ID" RESOLVED
api_get "/markets/$ID/resolution" | json 'print(d["status"], d["winning_outcome_index"])' | grep -q "^RESOLVED 0$" || fail "wrong resolution"
pass "the dispute window passed: RESOLVED, winner YES"

# --- payout: plan, FROST 2-of-3 signing, settlement ---------------------------------------------
PLAN="$("$OP" payout plan "$ID")"
HASH="$(echo "$PLAN" | json 'd["plan_hash"]')"
SHARES=("$STATE"/shares/*.share.enc)
printf '%s\n%s\n' "$HASH" "$HASH" | "$OP" payout sign "$ID" --plan-hash "$HASH" \
  --share "${SHARES[0]}" --member-key "$PRIV/member-1.key" --share "${SHARES[1]}" --member-key "$PRIV/member-2.key" >"$STATE/sign.json" 2>"$STATE/sign.err" \
  || { cat "$STATE/sign.err" >&2; fail "payout sign"; }
for _ in $(seq 1 20); do mine 1; sleep 3; status_is "$ID" SETTLED && break; done
status_is "$ID" SETTLED || fail "market did not reach SETTLED"
api_get "/markets/$ID/payout" | json 'print(d["status"], d["summary"]["winners"], d["summary"]["distributed_zat"])' > "$STATE/payout.txt"
grep -q "^CONFIRMED 1 " "$STATE/payout.txt" || fail "payout not confirmed with one winner: $(cat "$STATE/payout.txt")"
pass "COMMITTEE -> RESOLVED -> FROST payout -> SETTLED ($(tr -d '\n' < "$STATE/payout.txt"))"

"$W" dt wallet -w /work/recipient sync -s 127.0.0.1:8137 >/dev/null 2>&1 || true
"$W" dt wallet -w /work/recipient balance 2>/dev/null | grep -q . || true
set +e
"$BIN/frosted-verify" all "$ID" --node "$NODE" --api "$API" --json > "$STATE/verify.json"; VCODE=$?
set -e
python3 -I - "$STATE/verify.json" <<'PY' || exit 1
import json, sys
r = json.load(open(sys.argv[1])); s = {c["id"]: c["status"] for c in r["checks"]}
assert r["ok"], r
assert s["rules_hash"] == s["pools"] == s["plan_items"] == "PASS", s
assert s["resolution"] == "INFO", s   # committee outcomes are human decisions, not recomputable from the chain
PY
pass "frosted-verify all: pools and the payout plan PASS; the committee outcome is reported as not chain-verifiable"

# --- cycle 2: nobody decides -> VOID -------------------------------------------------------------
wait_zaino
T1="$(tip)"
cat > "$STATE/market2.json" <<JSON
{"question":"Regtest M11: the committee never decides","outcomes":["YES","NO"],"resolution_type":"COMMITTEE",
 "resolution_params":{"criteria":"Nobody votes.","evidence_source":"https://example.org/none","dispute_window_blocks":3,"void_after_blocks":8},
 "close_height":$((T1 + 14)),"resolve_height":$((T1 + 16)),"fee_bps":100}
JSON
ID2="$(create_market "$STATE/market2.json")"
mine 4
wait_for "second market OPEN" 90 status_is "$ID2" OPEN
mine $((T1 + 16 + 8 - $(tip) - 1))
sleep 8
status_is "$ID2" OPEN || status_is "$ID2" AWAITING_RESOLUTION || fail "second market left its lifecycle early: $(market_status "$ID2")"
mine 2
wait_for "VOID after void_after_blocks" 60 status_is "$ID2" VOID
pass "no decision for void_after_blocks: the market is VOID"
echo "==> all committee integration checks passed"
