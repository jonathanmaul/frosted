import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import Page from "@/app/verify/[id]/page";
import { ServerCheck } from "@/components/verify/ServerCheck";
import { verifyCommands, type VerifyReport } from "@/lib/verify";

const ID = "a".repeat(32);
const report: VerifyReport = {
  market_id: ID, command: "all", node_tip: 1034, api_height: 1030, ok: false,
  trust: "Server-side check, for convenience only. Do not trust it: a dishonest server could print PASS. Run frosted-verify against your own node.",
  checks: [
    { id: "rules_hash", name: "rules_hash = BLAKE2b-256(JCS(market_definition))", status: "PASS", detail: "abc" },
    { id: "pools", name: "Pools per outcome match the chain", status: "FAIL", detail: "The API's value differs from the verifier's recomputation.", diff: { verifier: { pool_total_zat: 3000000 }, api: { pool_total_zat: 3000001 } } },
    { id: "resolution", name: "Chain-Verified resolution matches the chain", status: "SKIP", detail: "No resolution has been published yet." },
  ],
};
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("S-10 Verify", () => {
  it("shows the exact CLI commands for the market and the offline Envelope Proof instruction", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: true, json: async () => ({ market_definition: { question: "Pool target?" } }) })));
    render(await Page({ params: Promise.resolve({ id: ID }) }));
    expect(screen.getByText(`frosted-verify all ${ID} --node http://127.0.0.1:8232`)).toBeTruthy();
    expect(screen.getByText(`frosted-verify pools ${ID} --node http://127.0.0.1:8232`)).toBeTruthy();
    expect(screen.getByText(/frosted-verify envelope-proof proof\.json/)).toBeTruthy();
    expect(screen.getByText(/Do not trust it/)).toBeTruthy();
    expect(verifyCommands(ID, "http://n").every((c) => c.command.includes("--node http://n"))).toBe(true);
  });

  it("runs the server check only when asked, and prints text verdicts with the diff of a failure", async () => {
    const fetchMock = vi.fn(async () => ({ ok: true, json: async () => report }));
    vi.stubGlobal("fetch", fetchMock);
    render(<ServerCheck marketId={ID} />);
    expect(fetchMock).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Run server-side check"));
    await screen.findByText(/Result:/);
    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toMatch(new RegExp(`/markets/${ID}/verify$`));
    expect(init.credentials).toBe("omit");
    expect(screen.getByText("[PASS]")).toBeTruthy(); expect(screen.getByText("[FAIL]")).toBeTruthy(); expect(screen.getByText("[SKIP]")).toBeTruthy();
    expect(screen.getByText('{"pool_total_zat":3000000}')).toBeTruthy(); expect(screen.getByText('{"pool_total_zat":3000001}')).toBeTruthy();
    expect(screen.getByText("FAIL", { selector: "strong" })).toBeTruthy();
    expect(screen.getAllByText(/dishonest server could print PASS/)).toHaveLength(2);
    expect(screen.getByText(/Server node at block 1,034, API data at block 1,030/)).toBeTruthy();
  });

  it("reports an unavailable check without inventing a verdict", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => ({ ok: false, json: async () => ({ error: { code: "NODE_UNAVAILABLE", message: "Server-side verification is unavailable: node unreachable" } }) })));
    render(<ServerCheck marketId={ID} />);
    fireEvent.click(screen.getByText("Run server-side check"));
    expect((await screen.findByRole("alert")).textContent).toContain("unavailable");
    expect(screen.queryByText("[PASS]")).toBeNull(); expect(screen.queryByText(/Result:/)).toBeNull();
  });

  it("handles a network failure", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => { throw new Error("offline"); }));
    render(<ServerCheck marketId={ID} />);
    fireEvent.click(screen.getByText("Run server-side check"));
    expect((await screen.findByRole("alert")).textContent).toContain("unreachable");
  });
});
