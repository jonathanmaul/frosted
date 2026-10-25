import { readFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";

describe("snapshot mode", () => {
  afterEach(() => { vi.unstubAllEnvs(); vi.resetModules(); });

  it("serves only recorded API responses and never calls the network", async () => {
    vi.stubEnv("NEXT_PUBLIC_SNAPSHOT_MODE", "true");
    const fetchSpy = vi.spyOn(globalThis, "fetch");
    const { fetchMarkets, fetchTip, featuredMarket, getJson } = await import("@/lib/api");
    const markets = await fetchMarkets();
    expect(markets?.length).toBeGreaterThan(0);
    expect(await fetchTip()).toBeGreaterThan(0);
    expect(featuredMarket(markets)?.pool_total_zat).toBeGreaterThan(0); // settled but recorded pool
    expect(await getJson("/markets/not-recorded")).toBeNull();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it("is off by default", async () => {
    const { SNAPSHOT } = await import("@/lib/api");
    expect(SNAPSHOT).toBe(false);
  });

  it("holds public data only: no bet txids, payout addresses or envelope secrets (spec M12.5)", () => {
    const json = readFileSync("lib/snapshot.json", "utf8");
    for (const field of ["env_esk", "payout_ua", "bet_salt", "bet_id", "txid\":\"e398d80e"]) expect(json).not.toContain(field);
    // The recorded YES bet's own transaction id (printed by the demo wallet) must never be published.
    expect(json).not.toContain("e398d80e31e94224686b53897204dee6f57467fdcb1c50a005052349d024237c");
  });
});
