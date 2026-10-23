import "fake-indexeddb/auto";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { BetBuilder } from "@/components/bettor/BetBuilder";
import { Markets } from "@/components/bettor/Markets";
import { MarketDetail } from "@/components/bettor/MarketDetail";
import * as receipts from "@/lib/receipts";
import { getJson } from "@/lib/api";
import { exactZec, parseAmount, type Market } from "@/lib/bettor";
import { checkPlan, importReceipts, listReceipts, matchReceipt, parseReceipts, type BetReceipt } from "@/lib/receipts";

vi.mock("@/lib/motion", () => ({ useReducedMotion: () => true }));
const mocks = vi.hoisted(() => ({ build: vi.fn(), parsed: vi.fn(), saved: vi.fn() }));
vi.mock("@/lib/wasm", () => ({ loadGlassWasm: async () => ({ build_bet: mocks.build, parse_payout_ua: mocks.parsed }) }));
vi.mock("qrcode", () => ({ toDataURL: async () => "data:image/png;base64,AAAA" }));
const market = { status: "OPEN", rules_hash: "abc", announce_height: 5, market_definition: { market_id: "a".repeat(32), question: "Pool target?", network: "regtest", min_bet_zat: 1000000, close_height: 100, resolve_height: 110, outcomes: [{ outcome_index: 0, label: "YES" }, { outcome_index: 1, label: "NO" }] } } as Market;
const receipt: BetReceipt = { bet_fingerprint: "1".repeat(16), market_id: "a".repeat(32), outcome_index: 0, amount_zat: 1000000, payout_ua: "uregtest1abc", bet_salt: "2".repeat(16), env_esk: "3".repeat(64), tagged: false, note_ref: null, created_at: 1, last_status: "WAITING", amount_mismatch: false };
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks(); vi.clearAllMocks(); });

describe("money and public API", () => {
  it("parses exact zatoshi, rejects unsafe or rounded inputs", () => {
    expect(parseAmount("0.00000001")).toBe(1); expect(parseAmount("0.01000001")).toBe(1000001);
    expect(exactZec(1000001)).toBe("0.01000001");
    for (const v of ["1e3", "-1", "0", "0.000000001", "21000000.00000001", "NaN", "1."]) expect(() => parseAmount(v)).toThrow();
  });
  it("uses credential-free GET only and tolerates failure", async () => {
    const fetch = vi.fn().mockResolvedValue({ ok: true, json: async () => ({ ok: true }) }); vi.stubGlobal("fetch", fetch);
    expect(await getJson("/markets")).toEqual({ ok: true });
    expect(fetch.mock.calls[0][1].credentials).toBe("omit"); expect(fetch.mock.calls[0][1].body).toBeUndefined();
    fetch.mockRejectedValueOnce(new Error("offline")); expect(await getJson("/markets")).toBeNull();
  });
  it("filters status and kind without inventing data", () => {
    render(<Markets initialTip={5} initialMarkets={[{ market_id: "a", question: "A?", status: "OPEN", resolution_type: "COMMITTEE", close_height: 10, pool_total_zat: 0, odds: [] }, { market_id: "b", question: "B?", status: "CLOSED", resolution_type: "POOL_THRESHOLD", close_height: 4, pool_total_zat: 0, odds: [] }]} />);
    fireEvent.change(screen.getByLabelText("Status"), { target: { value: "OPEN" } }); expect(screen.queryByText("B?")).toBeNull();
    fireEvent.change(screen.getByLabelText("Kind"), { target: { value: "chain_verified" } }); expect(screen.getByText("No markets match these filters.")).toBeTruthy();
  });
});
describe("private local receipts", () => {
  it("imports atomically, strips untrusted observations and protects an existing fingerprint", async () => {
    await importReceipts(JSON.stringify([{ ...receipt, last_status: "PAID", note_ref: "fake", amount_mismatch: true, extra_secret: "ignored" }]));
    expect(await listReceipts()).toEqual([receipt]);
    const other = { ...receipt, bet_fingerprint: "4".repeat(16) };
    await expect(importReceipts(JSON.stringify([other, { ...receipt, amount_zat: 2 }]))).rejects.toThrow("Conflicting");
    expect(await listReceipts()).toEqual([receipt]);
    expect(() => parseReceipts(JSON.stringify([{ ...receipt, amount_zat: -1 }]))).toThrow();
    expect(() => parseReceipts(JSON.stringify([{ ...receipt, created_at: 9000000000000000 }]))).toThrow();
  });
  it("matches counted notes, detects wallet-edited amount, and checks receiver commitment", () => {
    const bet = { note_ref: "5".repeat(16), bet_fingerprint: receipt.bet_fingerprint, outcome_index: 0, amount_zat: 1000000, block_height: 12, status: "COUNTED", tagged: false };
    const counted = matchReceipt(receipt, [bet]); expect(counted.last_status).toBe("COUNTED"); expect(counted.note_ref).toBe(bet.note_ref);
    expect(matchReceipt(receipt, [{ ...bet, amount_zat: 2000000 }]).amount_mismatch).toBe(true);
    expect(matchReceipt(receipt, [bet, bet]).last_status).toBe("AMBIGUOUS");
    const item = { note_ref: bet.note_ref, bet_fingerprint: receipt.bet_fingerprint, kind: "WIN", amount_zat: 1900000, receiver_commitment: "commit" };
    expect(checkPlan(counted, [item], "commit")).toContain("WIN: 1900000 zat");
    expect(checkPlan(counted, [item], "different")).toContain("Mismatch");
    expect(checkPlan(counted, [{ ...item, kind: "UNCLAIMABLE", amount_zat: 0, receiver_commitment: null }], "commit")).toContain("UNCLAIMABLE: 0 zat");
  });
});
describe("four-step local builder", () => {
  async function toSeal() {
    fireEvent.click(screen.getByText("Continue")); fireEvent.click(screen.getByText("Continue"));
    fireEvent.change(screen.getByLabelText(/Payout Unified Address/), { target: { value: "uregtest1private" } });
    fireEvent.click(screen.getByText("Continue")); await screen.findByText("04 / Review & seal");
  }
  it("saves receipt before QR, sends no secret requests, and keeps four steps", async () => {
    const requests: unknown[] = []; vi.stubGlobal("fetch", (...args: unknown[]) => { requests.push(args); return Promise.reject(); });
    mocks.parsed.mockReturnValue({ ok: true }); mocks.build.mockReturnValue({ memo_text: "FRST1:secret", zip321_uri: "zcash:private", bet_fingerprint: "6".repeat(16), bet_salt: "7".repeat(16), env_esk: "8".repeat(64) });
    render(<BetBuilder market={market} />); expect(screen.getAllByRole("listitem")).toHaveLength(4);
    await toSeal(); fireEvent.click(screen.getByText("Seal & save receipt"));
    await screen.findByText("Receipt saved on this device."); expect((await listReceipts()).some(r => r.bet_fingerprint === "6".repeat(16))).toBe(true);
    expect(screen.getByRole("img")).toBeTruthy(); expect(requests).toEqual([]);
  });
  it("shows UA validation and refuses a QR", async () => {
    mocks.parsed.mockReturnValue({ ok: false, error: "PAYOUT_ADDRESS_NO_ORCHARD" });
    render(<BetBuilder market={market} />); fireEvent.click(screen.getByText("Continue")); fireEvent.click(screen.getByText("Continue"));
    await waitFor(() => expect(screen.getByLabelText(/Payout Unified Address/)).toBeTruthy());
    fireEvent.click(screen.getByText("Continue")); await screen.findByRole("alert"); expect(screen.getByRole("alert").textContent).toBe("PAYOUT_ADDRESS_NO_ORCHARD"); expect(screen.queryByRole("img")).toBeNull();
  });
  it("refuses to reveal a payable URI when local persistence fails", async () => {
    mocks.parsed.mockReturnValue({ ok: true });
    vi.spyOn(receipts, "saveReceipt").mockRejectedValueOnce(new Error("Storage quota exceeded"));
    render(<BetBuilder market={market} />); await toSeal(); fireEvent.click(screen.getByText("Seal & save receipt"));
    await screen.findByText("Storage quota exceeded"); expect(screen.queryByRole("img")).toBeNull(); expect(screen.queryByLabelText("Payment URI")).toBeNull();
  });
  it("disables betting outside OPEN", () => { render(<BetBuilder market={{ ...market, status: "CLOSED" }} />); expect(screen.getByText(/Closed at block/)).toBeTruthy(); expect(screen.queryByText("Continue")).toBeNull(); });
});

it("refreshes detail odds on the API's cursor-only SSE event and shows empty pools as —", async () => {
  let event: (() => void) | undefined;
  vi.stubGlobal("EventSource", class { onopen = null; onerror = null; addEventListener(name: string, cb: () => void) { if (name === "odds_updated") event = cb; } close() {} });
  const def = { ...market.market_definition, resolution_type: "POOL_THRESHOLD", resolution_params: {}, fee_bps: 100, uivk: "uivktest" };
  vi.stubGlobal("fetch", vi.fn(async (url: string) => ({ ok: true, json: async () => url.endsWith("/payout") || url.endsWith("/resolution") ? null : url.endsWith("/odds") ? { pool_total_zat: 100000000, outcomes: [{ outcome_index: 0, pool_zat: 75000000, probability: .75 }, { outcome_index: 1, pool_zat: 25000000, probability: .25 }] } : url.includes("/bets") ? { bets: [] } : { ...market, market_definition: def } })));
  render(<MarketDetail initialMarket={{ ...market, market_definition: def }} initialOdds={{ at_height: 5, pool_total_zat: 0, outcomes: [], pending_bets: 0 }} initialBets={[]} />);
  expect(screen.getByRole("img").getAttribute("aria-label")).toBe("YES —, NO —");
  await act(async () => { await event!(); }); await waitFor(() => expect(screen.getByRole("img").getAttribute("aria-label")).toBe("YES 75%, NO 25%"));
});
