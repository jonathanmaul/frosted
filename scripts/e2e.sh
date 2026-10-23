#!/usr/bin/env bash
# Real-browser tests (Playwright) against the real regtest stack.
# Starts a fresh, isolated stack (own state, ports and Next build), runs the tests, stops the stack.
#   scripts/e2e.sh                 run all specs
#   scripts/e2e.sh e2e/01-markets.spec.ts   run one spec (arguments go to `playwright test`)
# Needs: the Docker regtest stack, the faucet/recipient wallets, `cd web && pnpm install`, and Chromium
# (`cd web && pnpm exec playwright install chromium`). Screenshots land in web/test-results/shots.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export DEV_STATE="$ROOT/.regtest/e2e" API_PORT=18080 COORD_PORT=18090 WEB_PORT=13000 WEB_MODE=prod WEB_DIST=.next-e2e DEV_CLOSE_IN=250 DEV_MARKETS=2 DEV_BETS=0
export E2E_STATE="$DEV_STATE" E2E_API_URL="http://localhost:18080/v1" E2E_WEB_URL="http://localhost:13000"
cleanup() { "$ROOT/scripts/dev.sh" down >/dev/null 2>&1 || true; }
trap cleanup EXIT
"$ROOT/scripts/dev.sh" down >/dev/null 2>&1 || true
rm -rf "$DEV_STATE"
"$ROOT/scripts/dev.sh" up --detach
cd "$ROOT/web"
pnpm exec playwright test "$@"
