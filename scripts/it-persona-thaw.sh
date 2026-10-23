#!/usr/bin/env bash
# Regtest integration test for Persona and Thaw (change add-persona-thaw, M9, task 5.1).
# Real chain: Persona registration (FRSTP1) and Thaw-tagged bets (FRST1) are shielded payments
# that glass-indexer decrypts. Then frosted-api thaws the Persona over HTTP routing.
# The resolution and payout plan are pushed as the coordinator would; the FROST payout itself is
# proven by the M8 run.
# Needs: scripts/dev-up.sh stack and a funded faucet (scripts/regtest-wallet.sh setup).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/.regtest/it-persona-thaw"
W="$ROOT/scripts/regtest-wallet.sh"
export PUBLIC_DB_PATH="$STATE/public.db" FROSTED_NETWORK=regtest LOG_LEVEL=warn
rm -rf "$STATE" && mkdir -p "$STATE"

cargo build -q -p glass-indexer --example it_fixture
cargo build -q -p glass-indexer
cargo build -q -p frosted-api --example it_thaw
BIN="$ROOT/target/debug/glass-indexer"
FX="$ROOT/target/debug/examples/it_fixture"
THAW="$ROOT/target/debug/examples/it_thaw"
idx() { "$BIN" "$@"; }
hex32() { python3 -I -c 'import secrets;print(secrets.token_hex(32))'; }

pass() { echo "PASS $1"; }
fail() { echo "FAIL $1" >&2; exit 1; }

read -r UIVK A0 A1 _A2 < <("$FX" escrow "$(hex32)")
K_ROOT="$(hex32)"
echo "$K_ROOT" > "$STATE/k_root.txt"   # test key only; scripts/it-verify.sh reuses it
OTHER_K="$(hex32)"
read -r FRSTP1 PERSONA_ID < <("$FX" frstp1 "$K_ROOT")

"$ROOT/scripts/dev-up.sh" >/dev/null
TIP="$("$W" mine 1)"
MID="$(python3 -I -c 'import secrets;print(secrets.token_hex(16))')"
"$FX" market "$UIVK" "$A0" "$A1" "$TIP" $((TIP + 60)) $((TIP + 65)) 100 "$MID" > "$STATE/market.json"
idx market-add "$STATE/market.json" >/dev/null
read -r REG_UIVK REG_ADDR < <("$FX" registry)
idx registry-set "$REG_UIVK" "$REG_ADDR" "$TIP"
ANNOUNCE="$("$FX" frstm1 "$STATE/market.json")"
tagged() { "$FX" frst1-tagged "$MID" "$1" "$2" "$3" "$4"; } # key outcome amount epk-byte

echo "==> market $MID, persona $PERSONA_ID"
"$W" send "$A0" 4000000 "$(tagged "$K_ROOT" 0 4000000 03)" >/dev/null   # tagged bet BEFORE registration
"$W" send "$REG_ADDR" 10000 "$FRSTP1" >/dev/null                         # Persona registration
"$W" send "$A0" 10000 "$ANNOUNCE" >/dev/null                             # announcement opens the market
WIN_MEMO="$(tagged "$K_ROOT" 0 5000000 01)"
"$W" send "$A0" 5000000 "$WIN_MEMO" >/dev/null                           # B1: will win
"$W" send "$A1" 3000000 "$(tagged "$K_ROOT" 1 3000000 02)" >/dev/null    # B2: will lose
"$W" send "$A0" 5000000 "$WIN_MEMO" >/dev/null                           # B3: copy of B1's exact payload
"$W" send "$A0" 2000000 "$(tagged "$OTHER_K" 0 2000000 04)" >/dev/null   # someone else's tag
"$W" mine 3 >/dev/null
# Pass resolve_height plus the confirmations a resolution needs, so verifiers can read that block.
NOW="$("$W" mine 1)"
[ "$NOW" -le $((TIP + 70)) ] && "$W" mine $((TIP + 70 - NOW + 1)) >/dev/null
idx once >/dev/null
idx dump "$MID" > "$STATE/dump.json"

read -r WIN_NOTE < <(python3 -I - "$STATE/dump.json" "$PERSONA_ID" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
persona = [p for p in d["personas"] if p["persona_id"] == sys.argv[2]]
assert len(persona) == 1, "Persona not registered by the indexer"
bets = [b for b in d["bets"] if b["tagged"]]
assert len(bets) == 5, f"expected 5 tagged bets, got {len(bets)}"
assert all(b["status"] in ("COUNTED", "PENDING") for b in bets), bets
first = min(b["block_height"] for b in bets)
assert persona[0]["registered_height"] > first, "registration must come after the first bet"
five = sorted((b for b in bets if b["amount_zat"] == 5000000), key=lambda b: b["block_height"])
print(five[0]["note_ref"])
PY
)
pass "indexer registered the Persona and counted five tagged bets from the chain"

"$THAW" "$PUBLIC_DB_PATH" "$MID" 0 "$WIN_NOTE" 4900000 "$PERSONA_ID" "$K_ROOT" orion > "$STATE/report.json"

python3 -I - "$STATE/report.json" "$WIN_NOTE" <<'PY'
import json, sys
r = json.load(open(sys.argv[1])); win = sys.argv[2]
b = r["bets"]
assert r["alias"] == "orion" and len(b) == 4, f"expected 4 bets of this Persona, got {len(b)}"
assert [x["block_height"] for x in b] == sorted(x["block_height"] for x in b), "ordered by height"
assert b[0]["exclusion"] == "PRE_REGISTRATION" and b[0]["amount_zat"] == 4000000, b[0]
assert b[1]["note_ref"] == win and b[1]["result"] == "WON" and b[1]["payout_zat"] == 4900000 and b[1]["counted_in_reputation"], b[1]
assert b[2]["result"] == "LOST" and b[2]["counted_in_reputation"] and b[2]["outcome_index"] == 1, b[2]
assert b[3]["exclusion"] == "DUPLICATE_TAG" and not b[3]["counted_in_reputation"], b[3]
assert r["stats"] == {"settled": 2, "won": 1, "win_rate": 0.5, "roi": -0.388}, r["stats"]
print("PASS report lists the win and the loss, flags PRE_REGISTRATION and DUPLICATE_TAG, omits other tags")
PY
echo "==> all persona/thaw integration checks passed"
