#!/usr/bin/env bash
# Regtest integration test for glass-indexer (change add-glass-indexer, M4.2–M4.6).
# Needs: scripts/dev-up.sh stack and a funded faucet (scripts/regtest-wallet.sh setup).
# Each run uses a fresh test escrow (random key), so notes from earlier runs never interfere.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/.regtest/it-indexer"
W="$ROOT/scripts/regtest-wallet.sh"
export PUBLIC_DB_PATH="$STATE/public.db" FROSTED_NETWORK=regtest LOG_LEVEL=warn
rm -rf "$STATE" && mkdir -p "$STATE"

cargo build -q -p glass-indexer --example it_fixture
BIN="$ROOT/target/debug/glass-indexer"
FX="$ROOT/target/debug/examples/it_fixture"
idx() { "$BIN" "$@"; }

read -r UIVK A0 A1 A2 < <("$FX" escrow "$(python3 -I -c 'import secrets;print(secrets.token_hex(32))')")

check() { # market_id python-expression-over-d (dump JSON)  description
  idx dump "$1" | python3 -I -c "
import json,sys
d=json.load(sys.stdin); s=[b['status'] for b in d['bets']]
ok=($2)
print(('PASS ' if ok else 'FAIL ')+'''$3''')
sys.exit(0 if ok else 1)"
}

"$ROOT/scripts/dev-up.sh" >/dev/null
TIP="$("$W" mine 1)"
MID="$(python3 -I -c 'import secrets;print(secrets.token_hex(16))')"
"$FX" market "$UIVK" "$A0" "$A1" "$TIP" $((TIP + 40)) $((TIP + 45)) 100 "$MID" > "$STATE/market.json"
idx market-add "$STATE/market.json" >/dev/null
read -r REG_UIVK REG_ADDR < <("$FX" registry)
idx registry-set "$REG_UIVK" "$REG_ADDR" "$TIP"
VALID="$("$FX" frst1 "$MID")"
OTHER="$("$FX" frst1 "$(python3 -I -c 'import secrets;print(secrets.token_hex(16))')")"
ANNOUNCE="$("$FX" frstm1 "$STATE/market.json")"
FORGED="FRSTM1:$(printf '0%.0s' {1..64})"

echo "==> market $MID birthday $TIP"
"$W" send "$A1" 2000000 "$VALID" >/dev/null       # bet while ANNOUNCING (ADR-0009)
idx once >/dev/null
check "$MID" "d['status']=='ANNOUNCING' and s==['PENDING']" "bet during ANNOUNCING is PENDING"

"$W" send "$A0" 10000 "$ANNOUNCE" >/dev/null       # valid announcement
"$W" send "$A0" 20000 "$FORGED" >/dev/null         # forged announcement
"$W" send "$A0" 5000000 "$VALID" >/dev/null        # valid bet
"$W" send "$A0" 500000 "$VALID" >/dev/null         # below MIN_BET → IGNORED_DUST
"$W" send "$A0" 5000 "$VALID" >/dev/null           # ≤ sweep threshold → UNECONOMIC_DUST
"$W" send "$A2" 2000000 "$VALID" >/dev/null        # diversifier 2: not an outcome
"$W" send "$A1" 2000000 "hello" >/dev/null         # not FRST1
"$W" send "$A0" 2000000 "$OTHER" >/dev/null        # other market_id
"$W" send "$REG_ADDR" 10000 "$("$FX" frstp1 "$(printf '11%.0s' {1..32})" | cut -d' ' -f1)" >/dev/null
"$W" send "$REG_ADDR" 10000 "$("$FX" frstp1 "$(printf '11%.0s' {1..32})" | cut -d' ' -f1)" >/dev/null # duplicate
"$W" mine 3 >/dev/null
idx once >/dev/null
check "$MID" "d['status']=='OPEN' and d['announce_height'] is not None" "valid announcement opens the market"
check "$MID" "s.count('ANNOUNCEMENT')==1" "only the first valid FRSTM1 is the announcement"
check "$MID" "s.count('COUNTED')==2" "valid bets COUNTED after 3 confirmations"
check "$MID" "s.count('IGNORED_DUST')==1 and s.count('UNECONOMIC_DUST')==1" "dust classes"
check "$MID" "s.count('UNKNOWN_OUTCOME')==1" "unknown diversifier"
check "$MID" "s.count('INVALID_MEMO')==3" "forged FRSTM1, plain text memo, and other market_id are INVALID_MEMO"
check "$MID" "len(d['personas'])==1" "Persona registered once despite duplicate"
check "$MID" "d['events']>0" "events emitted"

# Restart: processing again must not duplicate anything.
BEFORE="$(idx dump "$MID" | python3 -I -c 'import json,sys;print(len(json.load(sys.stdin)["bets"]))')"
idx once >/dev/null
check "$MID" "len(d['bets'])==$BEFORE" "restart resumes without duplicates"

# Reorg: invalidate the block that holds a bet and mine a replacement chain. Zebra 6.4.2 does
# not return the invalidated block's transactions to the mempool, so the bet leaves the chain
# and must leave public.db (spec: "Reorg removes a bet").
LAST_BET_TX="$("$W" send "$A1" 3000000 "$VALID")"
idx once >/dev/null
rpc() { curl -s -H 'content-type: application/json' -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":$2}" http://127.0.0.1:18232; }
OLD_HASH="$(rpc getrawtransaction "[\"$LAST_BET_TX\",1]" | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["result"]["blockhash"])')"
OLD_HEIGHT="$(rpc getblock "[\"$OLD_HASH\",1]" | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["result"]["height"])')"
python3 -I -c "import sys; n=$(sqlite3 "$PUBLIC_DB_PATH" "SELECT COUNT(*) FROM bet WHERE bet_id LIKE '$LAST_BET_TX:%'"); print(('PASS ' if n==1 else 'FAIL ')+'bet indexed before reorg'); sys.exit(0 if n==1 else 1)"
rpc invalidateblock "[\"$OLD_HASH\"]" >/dev/null
"$W" mine 2 >/dev/null
idx once >/dev/null
NODE_HASH="$(rpc getblockhash "[$OLD_HEIGHT]" | python3 -I -c 'import json,sys;print(json.load(sys.stdin)["result"])')"
STORED_HASH="$(sqlite3 "$PUBLIC_DB_PATH" "SELECT hash FROM block WHERE height = $OLD_HEIGHT")"
COUNT="$(sqlite3 "$PUBLIC_DB_PATH" "SELECT COUNT(*) FROM bet WHERE bet_id LIKE '$LAST_BET_TX:%'")"
python3 -I -c "import sys; ok='$NODE_HASH'!='$OLD_HASH' and '$STORED_HASH'=='$NODE_HASH'; print(('PASS ' if ok else 'FAIL ')+'indexer followed the replacement block at the fork height'); sys.exit(0 if ok else 1)"
python3 -I -c "import sys; ok=$COUNT==0; print(('PASS ' if ok else 'FAIL ')+'bet from the replaced block removed'); sys.exit(0 if ok else 1)"
# Let the faucet wallet notice the reorg before further sends.
"$W" dt wallet -w /work/faucet sync -s 127.0.0.1:8137 >/dev/null 2>&1 || true

# Late bet after close_height.
CLOSE=$((TIP + 40))
NOW="$("$W" mine 1)"
[ "$NOW" -le "$CLOSE" ] && "$W" mine $((CLOSE - NOW + 1)) >/dev/null
"$W" send "$A0" 2000000 "$VALID" >/dev/null
idx once >/dev/null
check "$MID" "'LATE' in s and d['status'] in ('CLOSED','AWAITING_RESOLUTION')" "late bet and closed market"

# Announcement timeout: a market without announcement becomes VOID.
TIP2="$("$W" mine 1)"
MID2="$(python3 -I -c 'import secrets;print(secrets.token_hex(16))')"
"$FX" market "$UIVK" "$A0" "$A1" "$TIP2" $((TIP2 + 40)) $((TIP2 + 45)) 3 "$MID2" > "$STATE/market2.json"
idx market-add "$STATE/market2.json" >/dev/null
"$W" mine 4 >/dev/null
idx once >/dev/null
check "$MID2" "d['status']=='VOID'" "no announcement within announce_timeout_blocks → VOID"

echo "==> all indexer integration checks passed"
