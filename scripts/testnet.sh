#!/usr/bin/env bash
# Frosted against the public Zcash testnet, with no local node.
#
#   scripts/testnet.sh up              keys, committee, hot wallet; start indexer, API and coordinator
#   scripts/testnet.sh fund <zat>      send testnet coins from the `demo` wallet to the hot wallet
#   scripts/testnet.sh market [close_in_blocks] [question]   create a chain-verified market
#   scripts/testnet.sh bet <market_id> <0|1> <zat>           build a bet with the WASM builder and pay it
#   scripts/testnet.sh address         print the demo wallet address to paste as the payout address in the web Bet Builder
#   scripts/testnet.sh pay '<zcash:...>'   pay a ZIP-321 URI copied from the web Bet Builder from the `demo` wallet
#   scripts/testnet.sh payout [market_id]                    plan the payout and sign it with members 1 and 2 (2-of-3)
#   scripts/testnet.sh web             open the web app for this stack at http://localhost:3100
#   scripts/testnet.sh fresh           set the old markets and indexer state aside so a new market starts from the chain tip
#   scripts/testnet.sh status | logs <name> | down
#
# Needs: ~/.config/frosted/testnet.env with ZEBRA_RPC_URL (a testnet node on the NU7 chain; keep it
# out of the repo) and the `demo` wallet from `zcash-devtool wallet init -n test`
# (.regtest/testnet-demo). Light-client traffic goes to testnet.zec.rocks. Nothing here touches
# mainnet. Testnet blocks arrive about once a minute.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="${TESTNET_STATE:-$ROOT/.testnet/run}"
PRIV="$STATE/private"; LOGS="$STATE/logs"; PIDFILE="$STATE/pids"
API_PORT="${API_PORT:-28080}"; COORD_PORT="${COORD_PORT:-28090}"; WEB_PORT="${WEB_PORT:-3100}"
API="http://127.0.0.1:$API_PORT/v1"
BIN="$ROOT/target/debug"
ENVFILE="${FROSTED_TESTNET_ENV:-$HOME/.config/frosted/testnet.env}"
GRPC="${LIGHTWALLETD_GRPC_URL:-https://testnet.zec.rocks:443}"
EXPLORER="${EXPLORER_URL:-https://testnet.zecblock.com}"
DEMO_DIR="testnet-demo"

say() { echo "==> $*"; }
die() { echo "error: $*" >&2; exit 1; }
json() { python3 -I -c "import json,sys; d=json.load(sys.stdin); print($1)"; }
op() { COORDINATOR_BIND="127.0.0.1:$COORD_PORT" "$BIN/frosted-operator" "$@"; }
api_get() { curl -sf "$API$1"; }
dt() { docker run --rm -v "$ROOT/.regtest:/work" frosted/zcash-devtool:0b3436b "$@"; }
strip() { sed 's/\x1b\[[0-9;]*m//g'; }

[ -f "$ENVFILE" ] || die "missing $ENVFILE (ZEBRA_RPC_URL=https://...)"
# The node URL carries an access key: read it into the environment, never print it.
ZEBRA_RPC_URL="$(python3 -I -c 'import sys
for l in open(sys.argv[1]):
    k, _, v = l.strip().partition("=")
    if k == "ZEBRA_RPC_URL": print(v)' "$ENVFILE")"
[ -n "$ZEBRA_RPC_URL" ] || die "ZEBRA_RPC_URL is empty in $ENVFILE"
export ZEBRA_RPC_URL
export FROSTED_NETWORK=testnet LOG_LEVEL="${LOG_LEVEL:-info}"

rpc() { curl -sf -H 'content-type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-[]}}" "$ZEBRA_RPC_URL"; }
tip() { rpc getblockcount | json 'd["result"]'; }

spawn() { local name="$1"; shift; "$@" >"$LOGS/$name.log" 2>&1 & echo "$name:$!" >> "$PIDFILE"; }
kill_tree() { local p; for p in $(pgrep -P "$1" 2>/dev/null || true); do kill_tree "$p"; done; kill "$1" 2>/dev/null || true; }
stop_all() {
  [ -f "$PIDFILE" ] || return 0
  while IFS=: read -r name pid; do
    if [ -n "${pid:-}" ] && kill -0 "$pid" 2>/dev/null; then kill_tree "$pid"; echo "stopped $name"; fi
  done < "$PIDFILE"
  rm -f "$PIDFILE"
}
wait_for() { local what="$1" limit="$2"; shift 2; for _ in $(seq 1 "$limit"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done; die "timed out waiting for $what (see $LOGS)"; }

demo_ua() { dt wallet -w "/work/$DEMO_DIR" list-addresses 2>&1 | strip | grep -oE 'utest1[0-9a-z]+' | head -1; }
demo_sync() { dt wallet -w "/work/$DEMO_DIR" sync >/dev/null 2>&1 || true; }

# Scan the chain up to the tip, showing progress. Starting from a fresh market this takes seconds;
# catching up on an old market over a rate-limited public RPC can take hours.
catch_up() {
  PUBLIC_DB_PATH="$STATE/public.db" "$BIN/glass-indexer" once >"$LOGS/indexer-once.log" 2>&1 &
  local pid=$! t h
  t="$(tip)"
  while kill -0 "$pid" 2>/dev/null; do
    h="$(sqlite3 "$STATE/public.db" 'select coalesce(max(height),0) from block' 2>/dev/null || echo 0)"
    [ "$h" = 0 ] || say "indexer scanning: block $h of $t"
    sleep 10
  done
  wait "$pid" || die "indexer failed: $(sed -E 's#https?://[^ "]+#<url>#g' "$LOGS/indexer-once.log" | tail -3)"
}

cmd_up() {
  [ -x "$ROOT/wallet-helper/target/release/frosted-wallet-helper" ] || die "build the wallet helper first (see docs/TESTING.md)"
  [ -d "$ROOT/.regtest/$DEMO_DIR" ] || die "demo wallet missing: .regtest/$DEMO_DIR"
  mkdir -p "$LOGS"
  if [ -f "$PIDFILE" ]; then
    # A run that died leaves its pid file behind; only refuse while one of its processes is alive.
    local alive=0 name pid
    while IFS=: read -r name pid; do [ -z "${pid:-}" ] || ! kill -0 "$pid" 2>/dev/null || alive=1; done < "$PIDFILE"
    [ "$alive" = 0 ] || die "already running (scripts/testnet.sh down first)"
    rm -f "$PIDFILE"
  fi
  if lsof "$STATE/public.db" >/dev/null 2>&1; then die "another process is using $STATE/public.db (an old indexer?); stop it first"; fi
  say "checking the node (chain, NU7, tip)"
  local info; info="$(rpc getblockchaininfo)" || die "node RPC unreachable"
  echo "$info" | json '"chain=%s tip=%s nu7=%s" % (d["result"]["chain"], d["result"]["blocks"], [u["activationheight"] for u in d["result"]["upgrades"].values() if u["name"]=="NU7"])'
  say "building binaries"
  (cd "$ROOT" && cargo build -q -p payout-coordinator -p frosted-operator -p glass-indexer -p frosted-api -p frosted-verify)
  if [ ! -f "$PRIV/secrets.key" ]; then
    say "creating keys"
    op secrets init --dir "$PRIV" | json 'd["coordinator_pubkey"]' > "$STATE/pubkey"
  fi
  local treasury; treasury="$(demo_ua)"; [ -n "$treasury" ] || die "no demo address"
  : > "$PIDFILE"
  catch_up
  spawn indexer env PUBLIC_DB_PATH="$STATE/public.db" "$BIN/glass-indexer" run
  spawn api env PUBLIC_DB_PATH="$STATE/public.db" API_BIND="127.0.0.1:$API_PORT" API_PUBLIC_ORIGIN="http://localhost:$WEB_PORT" \
    COORDINATOR_PUBKEY="$(cat "$STATE/pubkey")" "$BIN/frosted-api"
  wait_for "frosted-api" 40 curl -sf "$API/health"
  # The helper hides failure reasons on purpose; this wrapper logs the operation and the exit code.
  cat > "$STATE/helper.sh" <<SH
#!/bin/sh
IN=\$(cat)
OP=\$(printf '%s' "\$IN" | python3 -I -c 'import json,sys; print(json.load(sys.stdin).get("op"))')
printf '%s' "\$IN" | "$ROOT/wallet-helper/target/release/frosted-wallet-helper" 2>>"$LOGS/helper.err"
RC=\$?
echo "\$OP rc=\$RC" >> "$LOGS/helper.log"
exit \$RC
SH
  chmod +x "$STATE/helper.sh"
  spawn coordinator env COORDINATOR_DB_PATH="$STATE/coordinator.db" COORDINATOR_INDEXER_DB_PATH="$STATE/coordinator-indexer.db" \
    SECRETS_KEY_FILE="$PRIV/secrets.key" COORDINATOR_KEY_FILE="$PRIV/coordinator.key" ENVELOPE_SK_FILE="$PRIV/envelope.sk" \
    HOT_WALLET_SEED_FILE="$PRIV/hot-wallet.seed" TREASURY_ADDRESS="$treasury" LIGHTWALLETD_GRPC_URL="$GRPC" \
    API_INGEST_URL="$API/ingest" WALLET_HELPER_BIN="$STATE/helper.sh" COORDINATOR_BIND="127.0.0.1:$COORD_PORT" \
    ESCROW_SYNC_INTERVAL_SECS=10 "$BIN/payout-coordinator"
  wait_for "payout-coordinator" 40 curl -s -o /dev/null "http://127.0.0.1:$COORD_PORT/markets/none"
  if [ ! -f "$STATE/shares/committee.json" ]; then
    say "creating the 2-of-3 committee (trusted dealer)"
    op committee init --share-dir "$STATE/shares" --member-key "$PRIV/member-1.key" --member-key "$PRIV/member-2.key" --member-key "$PRIV/member-3.key" >/dev/null
  fi
  if [ ! -f "$STATE/hot-address" ]; then
    say "creating the hot wallet"
    op hot-wallet init | json 'd["address"]' > "$STATE/hot-address"
  fi
  echo; echo "  API            $API"
  echo "  Hot wallet     $(cat "$STATE/hot-address")"
  echo "  Fund it:       scripts/testnet.sh fund 3000000"
  echo "  Then:          scripts/testnet.sh market"
}

cmd_fresh() {
  if [ -f "$PIDFILE" ]; then die "stop the stack first (scripts/testnet.sh down)"; fi
  if lsof "$STATE/public.db" >/dev/null 2>&1; then die "a process still uses $STATE/public.db"; fi
  local old="$STATE/../old-$(date +%Y%m%d-%H%M%S)"
  mkdir -p "$old"
  say "copying the old databases to $old"
  for f in public.db coordinator-indexer.db coordinator.db last-market market.json; do
    [ ! -e "$STATE/$f" ] || cp "$STATE/$f" "$old/"
  done
  rm -f "$STATE"/public.db* "$STATE"/coordinator-indexer.db* "$STATE/last-market"
  if [ -f "$STATE/coordinator.db" ]; then
    # Keys, committee and hot wallet stay. Only the old markets go, so no indexer replays their blocks.
    for t in market resolution payout committee_vote committee_round; do
      sqlite3 "$STATE/coordinator.db" "DELETE FROM $t;"
    done
  fi
  say "done: the next market starts from the current chain tip (old state kept in $old)"
}

cmd_fund() {
  local zat="${1:-3000000}"
  demo_sync
  say "sending $zat zat from the demo wallet to the hot wallet"
  dt wallet -w "/work/$DEMO_DIR" send -i "/work/$DEMO_DIR/identity.txt" --address "$(cat "$STATE/hot-address")" \
    --value "$zat" --memo "fund hot wallet" 2>&1 | strip | grep -E "^[0-9a-f]{64}$|rror" | tail -2
}

cmd_market() {
  local close_in="${1:-60}" question="${2:-Will the Ironwood pool hold at least 0.00000001 ZEC at the resolve block?}"
  local t; t="$(tip)"
  python3 -I - "$question" "$((t + close_in))" > "$STATE/market.json" <<'PY'
import json, sys
close = int(sys.argv[2])
print(json.dumps({"question": sys.argv[1], "outcomes": ["YES", "NO"], "resolution_type": "POOL_THRESHOLD",
  "resolution_params": {"pool": "ironwood", "comparator": "gte", "threshold_zat": 1},
  "close_height": close, "resolve_height": close + 5, "fee_bps": 100}))
PY
  say "creating the market (closes at block $((t + close_in)), about $((close_in * 75 / 60)) minutes)"
  local out
  for _ in 1 2 3 4; do
    if out="$(op market create --file "$STATE/market.json" 2>"$LOGS/create.err")"; then
      echo "$out" | json 'd["market_definition"]["market_id"]' | tee "$STATE/last-market"; return 0
    fi
    sleep 10
  done
  die "market create failed: $(cat "$LOGS/create.err")"
}

cmd_bet() {
  local id="${1:-$(cat "$STATE/last-market" 2>/dev/null || true)}" outcome="${2:-0}" zat="${3:-1000000}"
  [ -n "$id" ] || die "usage: scripts/testnet.sh bet <market_id> <0|1> <zat>"
  local ua; ua="$(demo_ua)"
  api_get "/markets/$id" > "$STATE/bet-market.json"
  cat > "$STATE/bet.mjs" <<'JS'
import { readFileSync } from "node:fs";
const [root, marketFile, outcome, amount, ua] = process.argv.slice(2);
const mod = await import(`${root}/web/public/wasm/zcash_glass_wasm.js`);
await mod.default({ module_or_path: readFileSync(`${root}/web/public/wasm/zcash_glass_wasm_bg.wasm`) });
const market = JSON.parse(readFileSync(marketFile, "utf8"));
const built = mod.build_bet({ market_definition: market.market_definition, outcome_index: Number(outcome), amount_zat: Number(amount), payout_ua: ua });
console.log(JSON.stringify({ address: market.market_definition.outcomes[Number(outcome)].address, memo: built.memo_text }));
JS
  local out; out="$(node "$STATE/bet.mjs" "$ROOT" "$STATE/bet-market.json" "$outcome" "$zat" "$ua")"
  demo_sync
  say "paying a bet of $zat zat on outcome $outcome"
  dt wallet -w "/work/$DEMO_DIR" send -i "/work/$DEMO_DIR/identity.txt" --address "$(echo "$out" | json 'd["address"]')" \
    --value "$zat" --memo "$(echo "$out" | json 'd["memo"]')" 2>&1 | strip | grep -E "^[0-9a-f]{64}$|rror" | tail -2
}

# The web Bet Builder seals the bet in the browser and shows a ZIP-321 URI; the `demo` wallet stands in
# for the bettor's own wallet app and pays it unchanged.
cmd_pay() {
  local uri="${1:-}"
  [ -n "$uri" ] || die "usage: scripts/testnet.sh pay '<zcash:... uri>'"
  local parsed addr zat memo
  parsed="$(python3 -I - "$uri" <<'PY'
import sys, base64, urllib.parse as u
p = u.urlparse(sys.argv[1])
if p.scheme != "zcash":
    sys.exit("not a zcash: payment URI")
q = u.parse_qs(p.query)
amount = round(float(q["amount"][0]) * 1e8)
memo = q.get("memo", [""])[0]
memo = base64.urlsafe_b64decode(memo + "=" * (-len(memo) % 4)).decode() if memo else ""
print(p.path); print(amount); print(memo)
PY
)" || die "could not read the payment URI"
  { read -r addr; read -r zat; read -r memo; } <<< "$parsed"
  case "$addr" in utest1*) ;; *) die "not a testnet unified address: ${addr:0:12}..." ;; esac
  demo_sync
  say "paying $zat zat to ${addr:0:24}..."
  local txid; txid="$(dt wallet -w "/work/$DEMO_DIR" send -i "/work/$DEMO_DIR/identity.txt" --address "$addr" --value "$zat" --memo "$memo" 2>&1 | strip | grep -oE '^[0-9a-f]{64}$' | tail -1)"
  [ -n "$txid" ] || die "payment failed (check the demo wallet balance)"
  echo "txid $txid"
  echo "the bet shows on the market page after it is mined (about a minute); bet txids are not published by Frosted"
}

cmd_payout() {
  local id="${1:-$(cat "$STATE/last-market" 2>/dev/null || true)}"
  [ -n "$id" ] || die "usage: scripts/testnet.sh payout <market_id>"
  local status; status="$(op payout status "$id" 2>/dev/null | json 'd.get("status")' 2>/dev/null || echo None)"
  local plan hash
  if [ "$status" = "None" ] || [ -z "$status" ]; then
    say "planning the payout"
    plan="$(op payout plan "$id")" || die "payout plan failed (is the market resolved? $plan)"
    hash="$(echo "$plan" | json 'd["plan_hash"]')"
    echo "$plan" | json '"  %s winners, %s refunds, distributed %s zat, operator fee %s zat" % (d["summary"]["winners"], d["summary"]["refunds"], d["summary"]["distributed_zat"], d["summary"]["operator_fee_zat"])'
  else
    hash="$(op payout status "$id" | json 'd["plan_hash"]')"
  fi
  local s1 s2
  s1="$(ls "$STATE"/shares/0100*.share.enc)"; s2="$(ls "$STATE"/shares/0200*.share.enc)"
  # The demo holds all three shares (ADR-0008). Each member approves by typing the plan hash.
  say "signing with members 1 and 2 (plan $hash)"
  printf '%s\n%s\n' "$hash" "$hash" | op payout sign "$id" --plan-hash "$hash" \
    --share "$s1" --member-key "$PRIV/member-1.key" --share "$s2" --member-key "$PRIV/member-2.key" 2>&1 | strip | tail -3
  op payout status "$id" | json '"payout %s: %s" % (d["status"], [t["txid"] for t in d["txs"]])'
}

cmd_web() {
  [ -d "$ROOT/web/node_modules" ] || die "web dependencies missing: cd web && pnpm install"
  lsof -iTCP:"$WEB_PORT" -sTCP:LISTEN >/dev/null 2>&1 && die "port $WEB_PORT is already in use"
  # The API only accepts browser requests from API_PUBLIC_ORIGIN; restart it if it was started for another port.
  if ! curl -s -o /dev/null -D - -H "Origin: http://localhost:$WEB_PORT" "$API/health" | grep -qi "access-control-allow-origin: http://localhost:$WEB_PORT"; then
    say "restarting the API for origin http://localhost:$WEB_PORT"
    local pid; pid="$(grep '^api:' "$PIDFILE" | tail -1 | cut -d: -f2)"; [ -z "$pid" ] || kill_tree "$pid"
    sleep 1
    spawn api env PUBLIC_DB_PATH="$STATE/public.db" API_BIND="127.0.0.1:$API_PORT" API_PUBLIC_ORIGIN="http://localhost:$WEB_PORT" \
      COORDINATOR_PUBKEY="$(cat "$STATE/pubkey")" "$BIN/frosted-api"
    wait_for "frosted-api" 40 curl -sf "$API/health"
  fi
  say "starting the web app on port $WEB_PORT (testnet data from $API)"
  spawn web env NEXT_DIST_DIR=.next-testnet NEXT_PUBLIC_API_URL="$API" NEXT_PUBLIC_NETWORK=testnet NEXT_PUBLIC_EXPLORER_URL="$EXPLORER" \
    bash -c "cd '$ROOT/web' && exec node_modules/.bin/next dev -p $WEB_PORT"
  wait_for "the web app" 90 curl -s -o /dev/null "http://localhost:$WEB_PORT"
  echo "  Web  http://localhost:$WEB_PORT/markets"
}

cmd_status() {
  echo "state: $STATE"
  if [ -f "$PIDFILE" ]; then
    while IFS=: read -r name pid; do
      if kill -0 "$pid" 2>/dev/null; then echo "  up    $name"; else echo "  DOWN  $name"; fi
    done < "$PIDFILE"
  else echo "  not running"; fi
  echo "node tip: $(tip)"
  if curl -sf "$API/health" >/dev/null 2>&1; then
    api_get /health | json '"api: indexer=%s tip=%s lag=%s" % (d["indexer_height"], d["tip_height"], d["lag_blocks"])'
    api_get /markets | json '"\n".join("  %-20s %s  %s" % (m["status"], m["market_id"], m["question"][:50]) for m in d["markets"])'
  fi
}

case "${1:-status}" in
  up) cmd_up ;;
  down) stop_all ;;
  fresh) cmd_fresh ;;
  fund) shift; cmd_fund "$@" ;;
  market) shift; cmd_market "$@" ;;
  bet) shift; cmd_bet "$@" ;;
  pay) shift; cmd_pay "${1:-}" ;;
  address) demo_ua ;;
  payout) shift; cmd_payout "$@" ;;
  web) cmd_web ;;
  status) cmd_status ;;
  logs) tail -n 60 -f "$LOGS/${2:?indexer|api|coordinator|helper}.log" | python3 -I -u -c 'import sys
u = sys.argv[1]
for l in sys.stdin: print(l.replace(u, "<node-rpc-url>"), end="", flush=True)' "$ZEBRA_RPC_URL" ;;
  *) sed -n '2,14p' "$0"; exit 2 ;;
esac
