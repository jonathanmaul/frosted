#!/usr/bin/env bash
# One command for the whole local regtest stack.
#
#   scripts/dev.sh [up]            start everything and keep running; Ctrl-C stops it all
#   scripts/dev.sh up --detach     start everything in the background and return when ready
#   scripts/dev.sh down            stop the daemons and the web server (Docker zebra/zaino stay up)
#   scripts/dev.sh status          what runs, on which URL, and the markets
#   scripts/dev.sh pay <zip321-uri> [--no-mine]   pay a Bet Builder / Persona URI from the faucet, then mine 3 blocks
#   scripts/dev.sh mine [n]        mine n blocks (default 1)
#   scripts/dev.sh market <chain|committee> "<question>" <threshold_zat|criteria> [close_in_blocks]   add one market
#   scripts/dev.sh op <args...>    run frosted-operator against this stack's coordinator
#   scripts/dev.sh recipient       print the recipient wallet's Orchard address (a payout address for bets)
#   scripts/dev.sh logs <name>     tail a log: indexer | api | coordinator | web
#   scripts/dev.sh reset           delete this stack's state (keys, databases, markets); the chain stays
#
# Started: zebrad + Zaino (Docker), glass-indexer, frosted-api, payout-coordinator, the web app.
# First run also creates keys, the 2-of-3 committee, a funded hot wallet and eight demo markets.
#
# Environment (all optional):
#   DEV_STATE=.regtest/dev   state directory         WEB_MODE=dev|prod   next dev (default) or build + start
#   API_PORT=8080  COORD_PORT=8090  WEB_PORT=3000    DEV_SEED=0  skip demo markets   DEV_MARKETS=8  how many (max 8)   DEV_BETS=0  leave the pools empty
#   DEV_CLOSE_IN=60          blocks until demo markets close (resolve is 5 blocks later)
#   WEB_DIST=.next           Next.js build directory (so a prod build never clobbers `next dev`)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="${DEV_STATE:-$ROOT/.regtest/dev}"
case "$STATE" in /*) ;; *) STATE="$ROOT/$STATE" ;; esac
PRIV="$STATE/private"; LOGS="$STATE/logs"; PIDFILE="$STATE/pids"
API_PORT="${API_PORT:-8080}"; COORD_PORT="${COORD_PORT:-8090}"; WEB_PORT="${WEB_PORT:-3000}"
WEB_MODE="${WEB_MODE:-dev}"; WEB_DIST="${WEB_DIST:-.next}"
NODE_RPC="${ZEBRA_RPC_URL:-http://127.0.0.1:18232}"
ZAINO_GRPC="${LIGHTWALLETD_GRPC_URL:-http://127.0.0.1:9067}"
CLOSE_IN="${DEV_CLOSE_IN:-60}"
# Browsers treat localhost and 127.0.0.1 as different origins; the web app and CORS both use localhost.
API="http://localhost:$API_PORT/v1"; WEB="http://localhost:$WEB_PORT"
BIN="$ROOT/target/debug"; W="$ROOT/scripts/regtest-wallet.sh"
export FROSTED_NETWORK=regtest LOG_LEVEL="${LOG_LEVEL:-warn}" COORDINATOR_BIND="127.0.0.1:$COORD_PORT"

say() { echo "==> $*"; }
die() { echo "error: $*" >&2; exit 1; }
rpc() { curl -sf -H 'content-type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-[]}}" "$NODE_RPC"; }
json() { python3 -I -c "import json,sys; d=json.load(sys.stdin); print($1)"; }
tip() { rpc getblockcount | json 'd["result"]'; }
mine() { [ "$1" -gt 0 ] && "$W" mine "$1" >/dev/null || true; }
op() { "$BIN/frosted-operator" "$@"; }
api_get() { curl -sf "$API$1"; }
wait_for() { # description seconds command...
  local what="$1" limit="$2"; shift 2
  for _ in $(seq 1 "$limit"); do "$@" >/dev/null 2>&1 && return 0; sleep 1; done
  die "timed out waiting for $what (see $LOGS)"
}

# ---- process bookkeeping -----------------------------------------------------------------------
kill_tree() { # pid: children first, then the process
  local p; for p in $(pgrep -P "$1" 2>/dev/null || true); do kill_tree "$p"; done
  kill "$1" 2>/dev/null || true
}
stop_all() {
  [ -f "$PIDFILE" ] || return 0
  while IFS=: read -r name pid; do
    if [ -n "${pid:-}" ] && kill -0 "$pid" 2>/dev/null; then kill_tree "$pid"; echo "stopped $name"; fi
  done < "$PIDFILE"
  rm -f "$PIDFILE"
}
spawn() { # name command...
  local name="$1"; shift
  "$@" >"$LOGS/$name.log" 2>&1 &
  echo "$name:$!" >> "$PIDFILE"
}
port_free() { ! lsof -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1; }

# ---- steps -------------------------------------------------------------------------------------
check_prereqs() {
  for t in docker node pnpm cargo python3 sqlite3 curl lsof; do command -v "$t" >/dev/null || die "$t is not installed"; done
  [ -f "$ROOT/web/public/wasm/zcash_glass_wasm_bg.wasm" ] || die "WASM missing: cd web && pnpm build:wasm"
  [ -x "$ROOT/wallet-helper/target/release/frosted-wallet-helper" ] || die "wallet helper missing: cargo build --release --manifest-path wallet-helper/Cargo.toml"
  { [ -d "$ROOT/.regtest/faucet" ] && [ -d "$ROOT/.regtest/recipient" ]; } || die "test wallets missing: scripts/regtest-wallet.sh setup"
  [ -d "$ROOT/web/node_modules" ] || die "web dependencies missing: cd web && pnpm install"
  for p in $API_PORT $COORD_PORT $WEB_PORT; do port_free "$p" || die "port $p is already in use (stop the other process, or run scripts/dev.sh down)"; done
}

build_all() {
  say "building binaries"
  (cd "$ROOT" && cargo build -q -p payout-coordinator -p frosted-operator -p glass-indexer -p frosted-api -p frosted-verify && cargo build -q -p glass-indexer --example it_fixture)
}

# Zaino indexes asynchronously; importing an escrow wallet needs its tree state at the current tip.
wait_zaino() {
  local want have; want="$(tip)"
  for _ in $(seq 1 60); do
    have="$("$W" dt wallet -w /work/faucet get-info -s 127.0.0.1:8137 2>/dev/null | json 'd["chain_tip_height"]' 2>/dev/null || echo 0)"
    [ "${have:-0}" -ge "$want" ] && return 0
    sleep 2
  done
  die "Zaino did not reach the node tip $want"
}

init_state() {
  mkdir -p "$LOGS"
  if [ ! -f "$PRIV/secrets.key" ]; then
    say "creating keys"
    op secrets init --dir "$PRIV" | json 'd["coordinator_pubkey"]' > "$STATE/pubkey"
  fi
  if [ ! -f "$STATE/public.db" ]; then
    PUBLIC_DB_PATH="$STATE/public.db" ZEBRA_RPC_URL="$NODE_RPC" "$BIN/glass-indexer" once >/dev/null
  fi
  if [ ! -f "$STATE/registry.done" ]; then
    # The Persona Registry is a fixed test key; the Persona screen pays registrations to it.
    read -r REG_UIVK REG_ADDR < <("$BIN/examples/it_fixture" registry)
    PUBLIC_DB_PATH="$STATE/public.db" ZEBRA_RPC_URL="$NODE_RPC" "$BIN/glass-indexer" registry-set "$REG_UIVK" "$REG_ADDR" "$(tip)" >/dev/null
    echo "$REG_ADDR" > "$STATE/registry.done"
  fi
}

start_daemons() {
  : > "$PIDFILE"
  local reg_addr; reg_addr="$(cat "$STATE/registry.done")"
  spawn indexer env PUBLIC_DB_PATH="$STATE/public.db" ZEBRA_RPC_URL="$NODE_RPC" "$BIN/glass-indexer" run
  spawn api env COORDINATOR_PUBKEY="$(cat "$STATE/pubkey")" API_BIND="127.0.0.1:$API_PORT" API_PUBLIC_ORIGIN="$WEB" \
    PUBLIC_DB_PATH="$STATE/public.db" ZEBRA_RPC_URL="$NODE_RPC" "$BIN/frosted-api"
  wait_for "frosted-api" 30 curl -sf "http://127.0.0.1:$API_PORT/v1/health" -o /dev/null
  # The wallet helper hides failure reasons on purpose; this wrapper logs only the op and exit code.
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
    HOT_WALLET_SEED_FILE="$PRIV/hot-wallet.seed" TREASURY_ADDRESS="$reg_addr" ZEBRA_RPC_URL="$NODE_RPC" \
    LIGHTWALLETD_GRPC_URL="$ZAINO_GRPC" API_INGEST_URL="http://127.0.0.1:$API_PORT/v1/ingest" \
    WALLET_HELPER_BIN="$STATE/helper.sh" ESCROW_SYNC_INTERVAL_SECS=3 "$BIN/payout-coordinator"
  wait_for "payout-coordinator" 30 curl -s -o /dev/null "http://127.0.0.1:$COORD_PORT/markets/none"
}

setup_committee_and_hot_wallet() {
  if [ ! -f "$STATE/shares/committee.json" ]; then
    say "creating the 2-of-3 committee (trusted dealer: one person holds all three shares in this demo)"
    op committee init --share-dir "$STATE/shares" --member-key "$PRIV/member-1.key" --member-key "$PRIV/member-2.key" --member-key "$PRIV/member-3.key" >/dev/null
  fi
  if [ ! -f "$STATE/hot-address" ]; then
    say "creating and funding the hot wallet"
    op hot-wallet init | json 'd["address"]' > "$STATE/hot-address"
    "$W" send "$(cat "$STATE/hot-address")" 50000000 "fund hot wallet" >/dev/null
    mine 3
  fi
}

create_market() { # json-file -> id. A failed escrow import writes nothing, so retrying is safe.
  local out
  for _ in 1 2 3; do
    wait_zaino
    if out="$(op market create --file "$1" 2>"$LOGS/create.err")"; then echo "$out" | json 'd["market_definition"]["market_id"]'; return 0; fi
    sleep 5
  done
  die "market create failed: $(cat "$LOGS/create.err")"
}

market_open() { [ "$(api_get "/markets/$1" | json 'd["status"]')" = OPEN ]; }

# market_json <chain|committee> <question> <threshold_zat|criteria> <close_in_blocks> -> JSON on stdout.
market_json() {
  local kind="$1" question="$2" param="$3" close_in="$4" t; t="$(tip)"
  python3 -I - "$kind" "$question" "$param" "$((t + close_in))" <<'PY'
import json, sys
kind, question, param, close = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4])
m = {"question": question, "outcomes": ["YES", "NO"], "close_height": close, "resolve_height": close + 5, "fee_bps": 100}
if kind == "chain":
    m["resolution_type"] = "POOL_THRESHOLD"
    m["resolution_params"] = {"pool": "ironwood", "comparator": "gte", "threshold_zat": int(param)}
else:
    m["resolution_type"] = "COMMITTEE"
    m["resolution_params"] = {"criteria": param, "evidence_source": "https://example.org/statement", "dispute_window_blocks": 6, "void_after_blocks": 60}
print(json.dumps(m))
PY
}

# The demo set: the first two are the ones scripts and e2e rely on; the rest add variety.
# Each row is kind|question|threshold_zat or criteria|extra blocks until close.
DEMO_MARKETS=(
  "chain|Will the Ironwood pool hold at least 0.00000001 ZEC at the resolve block?|1|0"
  "committee|Will the committee confirm the demo statement?|Two of three members agree on the outcome after reading the published statement.|0"
  "chain|Will the Ironwood pool hold at least 0.001 ZEC when this market resolves?|100000|5"
  "chain|Will the Ironwood pool hold at least 0.01 ZEC when this market resolves?|1000000|10"
  "chain|Will the Ironwood pool hold at least 0.1 ZEC when this market resolves?|10000000|15"
  "chain|Will the Ironwood pool hold at least 1 ZEC when this market resolves?|100000000|20"
  "committee|Will the published roadmap ship its first milestone on time?|Two of three members confirm the milestone is publicly released by the evidence source.|10"
  "committee|Will the demo incident report be published within the stated window?|Two of three members confirm a report appears at the evidence source before the market resolves.|15"
)

create_demo_market() { # row -> id. Writes the definition to $STATE/markets/<n>.json
  local kind question param extra; IFS='|' read -r kind question param extra <<< "$1"
  mkdir -p "$STATE/markets"
  local f; f="$STATE/markets/$(date +%s)-$RANDOM.json"
  market_json "$kind" "$question" "$param" "$((CLOSE_IN + extra))" > "$f"
  create_market "$f"
}

seed_markets() {
  [ "${DEV_SEED:-1}" = 1 ] || return 0
  if [ "$(api_get /markets | json 'len(d["markets"])')" -gt 0 ]; then return 0; fi
  local want="${DEV_MARKETS:-8}"
  [ "$want" -le "${#DEMO_MARKETS[@]}" ] || want="${#DEMO_MARKETS[@]}"
  say "creating $want demo markets (the first closes in $CLOSE_IN blocks, the others a little later)"
  wait_zaino
  local ids=() i
  for ((i = 0; i < want; i++)); do ids+=("$(create_demo_market "${DEMO_MARKETS[$i]}")"); done
  # An announcement the helper could not broadcast the first time is retried a few seconds later and
  # needs a block of its own, so keep mining one block at a time until every market is open.
  mine 4
  local id
  for id in "${ids[@]}"; do
    for i in $(seq 1 90); do
      market_open "$id" && break
      [ $((i % 6)) -eq 0 ] && mine 1
      sleep 2
    done
    market_open "$id" || die "market $id did not open"
  done
  seed_bets "${ids[@]}"
  printf '%s\n' "${ids[@]}" > "$STATE/demo-markets"
}

# seed_bets <market ids...>: pays real WASM-built bets from the faucet so the pools are not empty.
# Four bets per market, amounts scaled per market. A send mines one block; if the wallet still has
# no spendable note it waits for confirmations and retries.
seed_bets() {
  [ "${DEV_BETS:-1}" = 1 ] || return 0
  local ua; ua="$("$W" dt wallet -w /work/recipient list-addresses --receiver orchard | sed -n 's/.*\(uregtest1[0-9a-z]*\).*/\1/p' | head -1)"
  [ -n "$ua" ] || { say "no recipient address, skipping demo bets"; return 0; }
  cat > "$STATE/bet.mjs" <<'JS'
import { readFileSync } from "node:fs";
const [root, marketFile, outcome, amount, ua] = process.argv.slice(2);
const mod = await import(`${root}/web/public/wasm/zcash_glass_wasm.js`);
await mod.default({ module_or_path: readFileSync(`${root}/web/public/wasm/zcash_glass_wasm_bg.wasm`) });
const market = JSON.parse(readFileSync(marketFile, "utf8"));
const built = mod.build_bet({ market_definition: market.market_definition, outcome_index: Number(outcome), amount_zat: Number(amount), payout_ua: ua });
console.log(JSON.stringify({ address: market.market_definition.outcomes[Number(outcome)].address, memo: built.memo_text }));
JS
  say "placing demo bets so the pools hold real ZEC"
  local idx=0 id round out amt outcome try
  for id in "$@"; do api_get "/markets/$id" > "$STATE/seed-$idx.json"; idx=$((idx + 1)); done
  for round in 0 1 2 3; do
    idx=0
    for id in "$@"; do
      case $round in 0) outcome=0; amt=600000000 ;; 1) outcome=1; amt=300000000 ;; 2) outcome=0; amt=200000000 ;; *) outcome=1; amt=400000000 ;; esac
      amt=$((amt * (1 + idx % 3)))
      out="$(node "$STATE/bet.mjs" "$ROOT" "$STATE/seed-$idx.json" "$outcome" "$amt" "$ua" 2>/dev/null)" || { idx=$((idx + 1)); continue; }
      for try in 1 2 3 4; do
        "$W" send "$(echo "$out" | json 'd["address"]')" "$amt" "$(echo "$out" | json 'd["memo"]')" >/dev/null 2>&1 && break
        mine 3
      done
      idx=$((idx + 1))
    done
    mine 3
  done
  mine 4
}

cmd_market() { # market <chain|committee> "<question>" <threshold_zat|criteria> [close_in_blocks]
  local kind="${1:-}" question="${2:-}" param="${3:-}" close_in="${4:-$CLOSE_IN}"
  case "$kind" in chain|committee) ;; *) die "usage: scripts/dev.sh market <chain|committee> \"<question>\" <threshold_zat|criteria> [close_in_blocks]" ;; esac
  [ -n "$question" ] && [ -n "$param" ] || die "a question and a threshold (chain) or criteria (committee) are required"
  mkdir -p "$STATE/markets"
  local f="$STATE/markets/$(date +%s)-$RANDOM.json" id
  market_json "$kind" "$question" "$param" "$close_in" > "$f"
  wait_zaino
  id="$(create_market "$f")"
  mine 4
  local i
  for i in $(seq 1 90); do
    market_open "$id" && break
    [ $((i % 6)) -eq 0 ] && mine 1
    sleep 2
  done
  market_open "$id" || die "market $id did not open"
  echo "$id"
}

start_web() {
  local web="$ROOT/web" next="$ROOT/web/node_modules/.bin/next"
  say "starting the web app ($WEB_MODE)"
  if [ "$WEB_MODE" = prod ]; then
    (cd "$web" && NEXT_DIST_DIR="$WEB_DIST" NEXT_PUBLIC_API_URL="$API" "$next" build >"$LOGS/web-build.log" 2>&1) || die "web build failed (see $LOGS/web-build.log)"
    spawn web env NEXT_DIST_DIR="$WEB_DIST" NEXT_PUBLIC_API_URL="$API" bash -c "cd '$web' && exec '$next' start -p $WEB_PORT"
  else
    spawn web env NEXT_DIST_DIR="$WEB_DIST" NEXT_PUBLIC_API_URL="$API" bash -c "cd '$web' && exec '$next' dev -p $WEB_PORT"
  fi
  wait_for "the web app" 120 curl -sf "$WEB/" -o /dev/null
}

summary() {
  cat <<TXT

  Frosted is running (regtest, chain height $(tip))
    Web           $WEB
    API           $API
    Coordinator   http://127.0.0.1:$COORD_PORT  (loopback only; use: scripts/dev.sh op ...)
    State / logs  $STATE  ($LOGS)

  Pay a Bet Builder or Persona QR (copy the payment URI):   scripts/dev.sh pay '<zcash:...>'
  Payout address for a test bet:                            scripts/dev.sh recipient
  Move the chain (close / resolve / dispute window):        scripts/dev.sh mine 10
  Committee votes:  scripts/dev.sh op resolution propose <market_id> --outcome 0 --evidence-url https://example.org/e --member member-1
  Audit a market from your own node:                        target/debug/frosted-verify all <market_id> --node $NODE_RPC --api $API
TXT
}

cmd_up() {
  local detach=0; [ "${1:-}" = "--detach" ] && detach=1
  check_prereqs
  mkdir -p "$STATE/logs"
  say "starting zebrad + Zaino"
  "$ROOT/scripts/dev-up.sh" >/dev/null
  build_all
  init_state
  [ "$detach" = 1 ] || trap 'echo; stop_all' EXIT INT TERM
  start_daemons
  setup_committee_and_hot_wallet
  seed_markets
  start_web
  summary
  if [ "$detach" = 0 ]; then
    echo; echo "  Ctrl-C stops everything."
    wait
  fi
}

cmd_pay() {
  local uri="${1:-}" nomine="${2:-}"
  [ -n "$uri" ] || die "usage: scripts/dev.sh pay '<zcash:... uri>' [--no-mine]"
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
  local txid; txid="$("$W" send "$addr" "$zat" "$memo" | grep -oE '[0-9a-f]{64}' | tail -1)"
  [ "$nomine" = "--no-mine" ] || mine 3
  echo "paid $zat zat to ${addr:0:24}..., tip $(tip)"
  [ -z "$txid" ] || echo "txid $txid   (inspect: curl -s --data-binary '{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getrawtransaction\",\"params\":[\"$txid\",1]}' $NODE_RPC)"
}

cmd_status() {
  echo "state: $STATE"
  if [ -f "$PIDFILE" ]; then
    while IFS=: read -r name pid; do
      if kill -0 "$pid" 2>/dev/null; then echo "  up    $name (pid $pid)"; else echo "  DOWN  $name"; fi
    done < "$PIDFILE"
  else echo "  not running"; fi
  if curl -sf "$API/health" >/dev/null 2>&1; then
    api_get /markets | json '"\n".join("  %-20s %s  %s" % (m["status"], m["market_id"], m["question"][:60]) for m in d["markets"])'
  fi
}

case "${1:-up}" in
  up) shift || true; cmd_up "${1:-}" ;;
  down) stop_all ;;
  status) cmd_status ;;
  pay) shift; cmd_pay "${1:-}" "${2:-}" ;;
  mine) mine "${2:-1}"; echo "tip $(tip)" ;;
  market) shift; cmd_market "$@" ;;
  op) shift; op "$@" ;;
  recipient) "$W" dt wallet -w /work/recipient list-addresses --receiver orchard | sed -n 's/.*\(uregtest1[0-9a-z]*\).*/\1/p' | head -1 ;;
  logs) tail -n 60 -f "$LOGS/${2:?indexer|api|coordinator|web}.log" ;;
  reset) if [ -f "$PIDFILE" ]; then die "stop the stack first: scripts/dev.sh down"; fi; rm -rf "$STATE"; echo "removed $STATE" ;;
  *) sed -n '2,20p' "$0"; exit 2 ;;
esac
