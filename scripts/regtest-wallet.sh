#!/usr/bin/env bash
# Regtest test wallets (zcash-devtool, run in Docker) for Frosted development.
#
#   scripts/regtest-wallet.sh setup        create `faucet` + `recipient` wallets and fund `faucet`
#   scripts/regtest-wallet.sh memo-test    send 0.01 ZEC with memo FRST1:test to recipient's
#                                          Orchard-only UA and fetch the full tx (M0.1, V-21)
#   scripts/regtest-wallet.sh zip321-test  pay a ZIP-321 URI with a 237-byte FRST1 memo and compare
#                                          the received memo byte for byte (M0.6 regtest half)
#   scripts/regtest-wallet.sh dt <args>    run zcash-devtool with arbitrary arguments
#
# Wallet state lives in .regtest/ (gitignored). Activation heights for the wallets are derived
# from zebrad's getblockchaininfo, which is authoritative (same rule Zaino follows).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/.regtest"
RPC_URL="${ZEBRA_RPC_URL:-http://127.0.0.1:18232}"
SERVER="127.0.0.1:8137" # Zaino, reached inside zebra's network namespace
COINBASE_MATURITY=100

mkdir -p "$STATE"

# --no-deps: never let `run` recreate zebra/zaino; the stack is managed by scripts/dev-up.sh.
dt() { docker compose -f "$ROOT/infra/docker-compose.yml" --profile tools run --rm -T --no-deps devtool "$@"; }

rpc() {
  curl -sf -H 'content-type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":${2:-[]}}" "$RPC_URL"
}

mine() { rpc generate "[$1]" >/dev/null; }
tip() { rpc getblockcount | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["result"])'; }

write_activation_heights() {
  rpc getblockchaininfo | python3 -I -c '
import json, sys
r = json.load(sys.stdin)["result"]
names = {"Overwinter": "overwinter", "Sapling": "sapling", "Blossom": "blossom",
         "Heartwood": "heartwood", "Canopy": "canopy", "NU5": "nu5", "NU6": "nu6",
         "NU6.1": "nu6_1", "NU6.2": "nu6_2", "NU6.3": "nu6_3", "NU7": "nu7"}
got = {names[u["name"]]: u["activationheight"] for u in r["upgrades"].values() if u["name"] in names}
print("# Derived from zebrad getblockchaininfo by scripts/regtest-wallet.sh. Do not edit.")
for key in names.values():
    print(f"{key} = {got[key]}" if key in got else f"{key} = \"never\"")
' > "$STATE/activation-heights.toml"
}

init_wallet() { # name
  if [ ! -f "$STATE/$1/keys.toml" ]; then
    dt wallet -w "/work/$1" init --name "$1" -i /work/identity.txt -n regtest \
      --activation-heights /work/activation-heights.toml -s "$SERVER" --birthday 1
  fi
}

# Zaino indexes asynchronously; wait until it serves zebrad's tip before syncing.
wait_for_zaino() {
  local want have
  want="$(tip)"
  for _ in $(seq 1 60); do
    have="$(dt wallet -w /work/faucet get-info -s "$SERVER" 2>/dev/null \
      | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["chain_tip_height"])' 2>/dev/null || echo 0)"
    [ "$have" -ge "$want" ] && return 0
    sleep 2
  done
  echo "Zaino tip $have did not reach zebrad tip $want" >&2
  return 1
}

wallet_height() {
  dt wallet -w "/work/$1" balance 2>/dev/null | sed -n 's/^ *Height: *\([0-9]*\).*/\1/p' | head -1
}

# Sync until the wallet's scanned height reaches zebrad's tip (Zaino can lag behind its own
# reported tip for a few seconds after `generate`).
sync_wallet() {
  local want have
  want="$(tip)"
  for _ in $(seq 1 15); do
    wait_for_zaino
    dt wallet -w "/work/$1" sync -s "$SERVER" >/dev/null 2>&1 || true
    have="$(wallet_height "$1")"
    [ "${have:-0}" -ge "$want" ] && return 0
    sleep 2
  done
  echo "wallet $1 stuck at height ${have:-?} (tip $want)" >&2
  return 1
}

set_miner() { # address, or empty for zebrad's default
  if [ -n "$1" ]; then export FROSTED_MINER_ADDRESS="$1"; else unset FROSTED_MINER_ADDRESS; fi
  "$ROOT/scripts/dev-up.sh" >/dev/null
}

setup() {
  unset FROSTED_MINER_ADDRESS
  "$ROOT/scripts/dev-up.sh"
  write_activation_heights
  init_wallet faucet
  init_wallet recipient

  local taddr
  taddr="$(dt wallet -w /work/faucet list-addresses --receiver transparent | sed -n 's/.*Receiver(transparent): \(t[0-9A-Za-z]*\).*/\1/p' | head -1)"
  [ -n "$taddr" ] || { echo "could not read faucet transparent address" >&2; exit 1; }

  # 1. Coinbase to the faucet's transparent address.
  echo "==> mining 20 blocks to faucet $taddr"
  set_miner "$taddr"
  mine 20
  # 2. Switch the coinbase back to zebrad's default address so later blocks do not create
  #    immature faucet UTXOs (zcash-devtool's shield would try to spend them), then mature.
  set_miner ""
  echo "==> mining $((COINBASE_MATURITY + 1)) blocks to mature the faucet coinbase"
  mine $((COINBASE_MATURITY + 1))
  sync_wallet faucet
  # 3. Shield to the wallet's shielded pool (Ironwood after NU6.3).
  echo "==> shielding faucet coinbase"
  dt wallet -w /work/faucet shield -i /work/identity.txt -s "$SERVER" -a "$taddr"
  mine 5
  sync_wallet faucet
  dt wallet -w /work/faucet balance | grep -E 'Height|Balance|Spendable'
}

memo_test() {
  local ua txid
  ua="$(dt wallet -w /work/recipient list-addresses --receiver orchard | sed -n 's/.*\(uregtest1[0-9a-z]*\).*/\1/p' | head -1)"
  [ -n "$ua" ] || { echo "could not read recipient Orchard-only UA" >&2; exit 1; }
  echo "==> recipient Orchard-only UA: $ua"
  sync_wallet faucet
  txid="$(dt wallet -w /work/faucet send -i /work/identity.txt -s "$SERVER" \
    --address "$ua" --value 1000000 --memo "FRST1:test" | tee /dev/stderr | grep -oE '[0-9a-f]{64}' | tail -1)"
  [ -n "$txid" ] || { echo "send did not return a txid" >&2; exit 1; }
  mine 3
  echo "==> txid $txid at tip $(tip)"
  rpc getrawtransaction "[\"$txid\", 0]" | python3 -I -c '
import json, sys
r = json.load(sys.stdin)
hexdata = r["result"]
print("full tx fetched:", len(hexdata) // 2, "bytes")'
  echo "$txid" > "$STATE/memo-test.txid"

  # Light wallets only see memos after fetching full transactions (`enhance`).
  sync_wallet recipient
  dt wallet -w /work/recipient enhance -s "$SERVER" >/dev/null 2>&1
  dt wallet -w /work/recipient list-tx | grep -E 'Memo:' | head -3
}

# M0.6 (regtest half): does a ZIP-321 payment request carrying a real 237-byte FRST1 memo
# survive a wallet's URI handling byte for byte? Uses the shared zcash-glass test vector.
zip321_test() {
  local ua memo uri txid got
  ua="$(dt wallet -w /work/recipient list-addresses --receiver orchard | sed -n 's/.*\(uregtest1[0-9a-z]*\).*/\1/p' | head -1)"
  memo="$(python3 -I -c 'import json; print(json.load(open("'"$ROOT"'/docs/test-vectors/memo.json"))["frst1"][0]["memo_text"])')"
  # ZIP-321 `memo` = base64url (no padding) of the raw memo bytes.
  uri="zcash:${ua}?amount=0.01&memo=$(printf '%s' "$memo" | base64 | tr '+/' '-_' | tr -d '=\n')"
  echo "==> memo ${#memo} bytes, sha256 $(printf '%s' "$memo" | shasum -a 256 | cut -c1-16)"
  echo "==> URI ${uri:0:60}... (${#uri} chars)"
  sync_wallet faucet
  txid="$(dt wallet -w /work/faucet pay -i /work/identity.txt -s "$SERVER" --disable-confirmation \
    --payment-uri "$uri" | grep -oE '[0-9a-f]{64}' | tail -1)"
  [ -n "$txid" ] || { echo "pay did not return a txid" >&2; exit 1; }
  mine 3
  sync_wallet recipient
  dt wallet -w /work/recipient enhance -s "$SERVER" >/dev/null 2>&1
  got="$(dt wallet -w /work/recipient list-tx | grep -A30 "$txid" | sed -n 's/.*Memo: Memo::Text("\(.*\)").*/\1/p' | head -1)"
  echo "==> txid $txid"
  if [ "$got" = "$memo" ]; then
    echo "PASS: received memo is identical (${#got} bytes)"
  else
    echo "FAIL: received memo differs"; echo "sent: $memo"; echo "got:  $got"; exit 1
  fi
}

case "${1:-}" in
  setup) setup ;;
  memo-test) memo_test ;;
  zip321-test) zip321_test ;;
  dt) shift; dt "$@" ;;
  *) echo "usage: $0 {setup|memo-test|dt <args>}" >&2; exit 2 ;;
esac
