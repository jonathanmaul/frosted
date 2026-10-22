#!/usr/bin/env bash
# M3.4: pay a build_bet ZIP-321 URI on regtest and compare received memo bytes.
# Needs the existing dev-up stack and funded regtest-wallet.sh setup wallets.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/.regtest/it-wasm"
W="$ROOT/scripts/regtest-wallet.sh"
mkdir -p "$STATE"

dt() { "$W" dt "$@"; }
tip() {
  curl -sf -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"getblockcount","params":[]}' \
    "${ZEBRA_RPC_URL:-http://127.0.0.1:18232}" \
    | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["result"])'
}
sync_wallet() {
  local want have
  want="$(tip)"
  for _ in $(seq 1 60); do
    dt wallet -w "/work/$1" sync -s 127.0.0.1:8137 > "$STATE/sync.log" 2>&1 || true
    have="$(dt wallet -w "/work/$1" balance | sed -n 's/^ *Height: *\([0-9]*\).*/\1/p' | head -1)"
    [ "${have:-0}" -ge "$want" ] && return 0
    sleep 2
  done
  echo "wallet $1 did not reach height $want; see $STATE/sync.log" >&2
  return 1
}

UA="$(dt wallet -w /work/recipient list-addresses --receiver orchard | sed -n 's/.*\(uregtest1[0-9a-z]*\).*/\1/p' | head -1)"
[ -n "$UA" ] || { echo 'recipient Orchard UA missing; run regtest-wallet.sh setup' >&2; exit 1; }
python3 -I - "$ROOT" "$STATE" "$UA" <<'PY'
import base64, json, pathlib, sys
root, state, ua = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3]
market = json.loads((root / 'docs/test-vectors/hash.json').read_text())['market_definition']
pk = json.loads((root / 'docs/test-vectors/envelope.json').read_text())['envelope_pk']
market['network'] = 'regtest'
market['envelope_pk'] = base64.urlsafe_b64encode(bytes.fromhex(pk)).decode().rstrip('=')
for outcome in market['outcomes']:
    outcome['address'] = ua
data = dict(market_definition=market, outcome_index=0, amount_zat=1000000, payout_ua=ua, k_root=None)
(state / 'input.json').write_text(json.dumps(data))
PY
cargo run --quiet -p zcash-glass-wasm --example build_bet -- "$STATE/input.json" > "$STATE/built.json"
URI="$(python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["zip321_uri"])' "$STATE/built.json")"
sync_wallet faucet
dt wallet -w /work/faucet pay -i /work/identity.txt -s 127.0.0.1:8137 \
  --disable-confirmation --payment-uri "$URI" > "$STATE/pay.log"
TXID="$(grep -oE '[0-9a-f]{64}' "$STATE/pay.log" | tail -1)"
[ -n "$TXID" ] || { echo "pay returned no txid; see $STATE/pay.log" >&2; exit 1; }
"$W" mine 3 >/dev/null
sync_wallet recipient
dt wallet -w /work/recipient enhance -s 127.0.0.1:8137 > "$STATE/enhance.log" 2>&1
dt wallet -w /work/recipient list-tx > "$STATE/received.txt"
python3 -I - "$STATE" "$TXID" <<'PY'
import hashlib, json, pathlib, re, sys
state, txid = pathlib.Path(sys.argv[1]), sys.argv[2]
expected = json.loads((state / 'built.json').read_text())['memo_text'].encode()
listing = (state / 'received.txt').read_text()
transaction = re.search(rf'^{txid}\n(.*?)(?=^[0-9a-f]{{64}}$|\Z)', listing, re.M | re.S)
assert transaction is not None, 'paid transaction is not in the recipient wallet'
match = re.search(r'Memo: Memo::Text\("([^"\n]*)"\)', transaction[1])
assert match is not None, 'paid transaction has no received text memo'
received = match[1].encode()
assert received == expected, 'received memo differs from build_bet output'
assert len(received) == 237 and received.startswith(b'FRST1:'), 'invalid FRST1 memo'
print(f'PASS: build_bet URI paid; {len(received)} received memo bytes identical')
print(f'txid {txid}; memo sha256 {hashlib.sha256(received).hexdigest()}')
PY
