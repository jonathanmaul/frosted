import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CommitteePanel } from "@/components/bettor/CommitteePanel";
import { OperatorStatus } from "@/components/bettor/OperatorStatus";
import { Settlement } from "@/components/bettor/Settlement";
import type { MarketDefinition, Resolution } from "@/lib/bettor";
import { committeeCountdown, committeeParams, countdownText, isWebUrl } from "@/lib/committee";

vi.mock("@/lib/motion", () => ({ useReducedMotion: () => true }));
const ID = "c".repeat(32);
const def = { version: 1, params_version: 1, birthday_height: 1, min_bet_zat: 1000000, refund_fee_zat: 5000, min_confirmations: 3, sweep_threshold_zat: 5000,
  announce_timeout_blocks: 100, envelope_pk: "k", treasury_address: "t", market_id: ID, question: "Will the committee say YES?", network: "regtest",
  close_height: 10, resolve_height: 20, resolution_type: "COMMITTEE", fee_bps: 100, uivk: "u",
  resolution_params: { criteria: "Official statement published by 20 Oct", evidence_source: "https://example.org/statements", dispute_window_blocks: 10, void_after_blocks: 50 },
  outcomes: [{ outcome_index: 0, label: "YES", address: "a" }, { outcome_index: 1, label: "NO", address: "b" }] } as MarketDefinition;
const vote = (member_id: string, action: string, url: string | null = "https://example.org/e") => ({ member_id, action, outcome_index: 0, evidence_url: url, created_at: 1 });
const proposed: Resolution = { market_id: ID, status: "PROPOSED", winning_outcome_index: null, votes: [vote("member-1", "PROPOSE"), vote("member-2", "CONFIRM")],
  evidence: { source: "committee", height: 21, block_hash: "hash-21" } };
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("committee countdown", () => {
  it("counts the dispute window from the block recorded in the evidence", () => {
    expect(committeeCountdown(def, proposed, 25)).toEqual({ kind: "dispute", endsAt: 31, blocksLeft: 6 });
    expect(committeeCountdown(def, proposed, 31)).toEqual({ kind: "dispute", endsAt: 31, blocksLeft: 0 });
    expect(countdownText(committeeCountdown(def, proposed, 25))).toBe("Dispute window ends at block 31: 6 blocks left. Any member can still withdraw.");
    expect(countdownText(committeeCountdown(def, proposed, 30))).toContain("1 block left");
    expect(countdownText(committeeCountdown(def, proposed, 40))).toContain("deadline reached");
  });
  it("counts to the void deadline while undecided, and stops once decided", () => {
    const waiting: Resolution = { ...proposed, status: "AWAITING_RESOLUTION", votes: [], evidence: null };
    expect(committeeCountdown(def, waiting, 30)).toEqual({ kind: "void", endsAt: 70, blocksLeft: 40 });
    expect(committeeCountdown(def, null, 5)).toEqual({ kind: "void", endsAt: 70, blocksLeft: 65 });
    expect(countdownText(committeeCountdown(def, null, 5))).toContain("refunded");
    for (const status of ["RESOLVED", "VOID"]) expect(committeeCountdown(def, { ...proposed, status }, 30)).toEqual({ kind: "none" });
  });
  it("never invents a count when the tip is unknown, and ignores chain-verified markets", () => {
    expect(committeeCountdown(def, proposed, null)).toEqual({ kind: "dispute", endsAt: 31, blocksLeft: null });
    expect(countdownText(committeeCountdown(def, proposed, null))).toContain("— blocks left");
    expect(committeeParams({ ...def, resolution_type: "POOL_THRESHOLD", resolution_params: {} })).toBeNull();
    expect(committeeCountdown({ ...def, resolution_type: "POOL_THRESHOLD" }, null, 5)).toEqual({ kind: "none" });
  });
  it("only makes http(s) evidence links clickable", () => {
    expect(isWebUrl("https://example.org/e")).toBe(true);
    for (const bad of ["javascript:alert(1)", "data:text/html,x", "ftp://x", "https://a b", ""]) expect(isWebUrl(bad)).toBe(false);
  });
});

describe("committee panel (S-05)", () => {
  it("shows PROPOSED with both votes, evidence links and the countdown", () => {
    render(<Settlement definition={def} resolution={proposed} payout={null} tip={25} />);
    expect(screen.getAllByText("Proposed", { selector: "span" })).toHaveLength(1); // the one status badge
    expect(screen.getByRole("status").textContent).toContain("6 blocks left");
    expect(screen.getByText("member-1 · Proposed · YES")).toBeTruthy();
    expect(screen.getByText("member-2 · Confirmed · YES")).toBeTruthy();
    const links = screen.getAllByRole("link", { name: "https://example.org/e" });
    expect(links).toHaveLength(2);
    expect(links[0].getAttribute("rel")).toBe("noopener noreferrer");
    expect(screen.getByText(/one person holds all three member keys/)).toBeTruthy();
    expect(screen.getByText("Official statement published by 20 Oct")).toBeTruthy();
  });
  it("shows the cleared state after a withdrawal and the void deadline", () => {
    render(<Settlement definition={def} resolution={{ ...proposed, status: "AWAITING_RESOLUTION", votes: [], evidence: null }} payout={null} tip={30} />);
    expect(screen.getByText(/No proposal yet/)).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("voided at block 70");
  });
  it("renders nothing for a chain-verified market and no countdown once resolved", () => {
    const { container } = render(<CommitteePanel definition={{ ...def, resolution_type: "POOL_THRESHOLD" }} resolution={null} tip={1} />);
    expect(container.textContent).toBe("");
    cleanup();
    render(<CommitteePanel definition={def} resolution={{ ...proposed, status: "RESOLVED", winning_outcome_index: 0 }} tip={40} />);
    expect(screen.queryByRole("status")).toBeNull();
  });
  it("lets an unsafe evidence URL stay plain text", () => {
    render(<CommitteePanel definition={def} resolution={{ ...proposed, votes: [vote("member-1", "PROPOSE", "javascript:alert(1)")] }} tip={25} />);
    expect(screen.getByText("javascript:alert(1)")).toBeTruthy();
    expect(screen.queryAllByRole("link")).toHaveLength(0);
  });
});

describe("operator status (S-11)", () => {
  it("shows the live countdown for a committee market and the operator commands", async () => {
    const market = { market_id: ID, question: def.question, status: "PROPOSED", resolution_type: "COMMITTEE", close_height: 10, pool_total_zat: 0, odds: [] };
    vi.stubGlobal("fetch", vi.fn(async (url: string) => ({ ok: true, json: async () =>
      url.endsWith("/markets") ? { markets: [market] } : url.endsWith("/chain/tip") ? { height: 28 } : url.endsWith("/resolution") ? proposed
      : url.endsWith("/payout") ? null : { market_definition: def, rules_hash: "r", status: "PROPOSED", announce_height: 2 } })));
    await act(async () => { render(<OperatorStatus initialRows={[{ market, payout: null, committee: { definition: def, resolution: proposed } }]} />); });
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("3 blocks left"));
    expect(screen.getByText(/frosted-operator resolution propose/)).toBeTruthy();
  });
});
