import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Settlement } from "@/components/bettor/Settlement";
import { MarketDetail } from "@/components/bettor/MarketDetail";
import { OperatorStatus } from "@/components/bettor/OperatorStatus";
import type { Market, MarketDefinition, Payout, Resolution } from "@/lib/bettor";

vi.mock("@/lib/motion", () => ({ useReducedMotion: () => true }));
const definition: MarketDefinition = { version: 1, params_version: 1, birthday_height: 1, min_bet_zat: 1000000,
  refund_fee_zat: 5000, min_confirmations: 3, sweep_threshold_zat: 5000, announce_timeout_blocks: 100,
  envelope_pk: "fixture-key", treasury_address: "fixture-treasury",
  market_id: "a".repeat(32), question: "Upgrade active?", network: "regtest", close_height: 10, resolve_height: 12,
  resolution_type: "UPGRADE_ACTIVE", resolution_params: { upgrade_name: "NU7" }, fee_bps: 100, uivk: "public-uivk",
  outcomes: [{ outcome_index: 0, label: "YES", address: "fixture-yes" }, { outcome_index: 1, label: "NO", address: "fixture-no" }] };
const resolution: Resolution = { market_id: definition.market_id, status: "RESOLVED", winning_outcome_index: 1, votes: [],
  evidence: { source: "getblockchaininfo", height: 12, block_hash: "resolve-block-hash", reported_upgrades: [{ name: "NU6.3", activation_height: 10 }] } };
const payout: Payout = { plan_hash: "public-plan-hash", status: "SIGNING",
  plan: { market_id: definition.market_id, plan_kind: "STANDARD", plan_cutoff_height: 15, result: 1, operator_fee_zat: 10000,
    items: [{ note_ref: "public-note", bet_fingerprint: "fp", kind: "WIN", amount_zat: 990000, receiver_commitment: "public-commitment" }] },
  summary: { winners: 1, refunds: 0, unclaimable: 0, swept: 0, uneconomic_dust: 0, operator_fee_zat: 10000, tx_fee_zat: 10000, distributed_zat: 990000 },
  signing: { sessions_done: 9, sessions_total: 11, threshold: 2 }, txs: [{ txid: "payout-transaction", status: "CONFIRMED", confirmed_height: 18 }] };
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

it("shows public evidence, exact item amounts and real signing progress", () => {
  render(<Settlement definition={definition} resolution={resolution} payout={payout} />);
  expect(screen.getByText("Outcome: NO")).toBeTruthy();
  expect(screen.getByText("NU6.3 / Activation block 10")).toBeTruthy();
  expect(screen.getByText("resolve-block-hash")).toBeTruthy();
  const progress = screen.getByRole("progressbar");
  expect(progress.getAttribute("value")).toBe("9"); expect(progress.getAttribute("max")).toBe("11");
  expect(screen.getByText("0.00990000")).toBeTruthy();
  expect(screen.getByText("payout-transaction")).toBeTruthy();
  expect(screen.getByText("Confirmed at block 18")).toBeTruthy();
});

it("keeps unknown resolution and payout values unavailable", () => {
  render(<Settlement definition={definition} resolution={null} payout={null} />);
  expect(screen.getByText("No public resolution published.")).toBeTruthy();
  expect(screen.queryByRole("progressbar")).toBeNull();
  expect(screen.getByText("No public payout plan published.")).toBeTruthy();
});

it("updates settlement on payout SSE without adding private operator actions", async () => {
  let update: (() => Promise<void>) | undefined;
  vi.stubGlobal("EventSource", class { onopen = null; onerror = null; addEventListener(name: string, cb: () => Promise<void>) { if (name === "payout_updated") update = cb; } close() {} });
  const market: Market = { market_definition: definition, rules_hash: "rules", status: "RESOLVED", announce_height: 1 };
  const fetch = vi.fn(async (url: string) => ({ ok: true, json: async () => url.endsWith("/payout") ? payout : url.endsWith("/resolution") ? resolution : url.endsWith("/odds") ? { pool_total_zat: 0, outcomes: [] } : url.includes("/bets") ? { bets: [] } : market }));
  vi.stubGlobal("fetch", fetch);
  render(<MarketDetail initialMarket={market} initialBets={[]} initialOdds={null} />);
  await act(async () => { await update!(); });
  await waitFor(() => expect(screen.getByText("plan_hash: public-plan-hash")).toBeTruthy());
  expect(screen.getByRole("progressbar").getAttribute("value")).toBe("9");
  for (const [, options] of fetch.mock.calls as unknown as [string, RequestInit][]) { expect(options.body).toBeUndefined(); expect(options.credentials).toBe("omit"); }
});

it("operator screen displays public progress and exposes no action controls", () => {
  render(<OperatorStatus initialRows={[{ market: { market_id: definition.market_id, question: definition.question, status: "RESOLVED", resolution_type: "UPGRADE_ACTIVE", close_height: 10, pool_total_zat: 1000000, odds: [] }, payout }]} />);
  expect(screen.getByRole("progressbar").getAttribute("max")).toBe("11");
  expect(screen.queryByRole("button")).toBeNull();
  expect(screen.queryByRole("textbox")).toBeNull();
});
