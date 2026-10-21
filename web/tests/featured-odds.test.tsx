import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useFeaturedOdds } from "@/lib/useFeaturedOdds";

class MockEventSource {
  static last: MockEventSource | null = null;
  listeners: Record<string, (e: MessageEvent) => void> = {};
  constructor(public url: string) {
    MockEventSource.last = this;
  }
  addEventListener(name: string, fn: (e: MessageEvent) => void) {
    this.listeners[name] = fn;
  }
  close() {}
  emit(name: string, data: unknown) {
    this.listeners[name]?.(new MessageEvent(name, { data: JSON.stringify(data) }));
  }
}

afterEach(() => vi.unstubAllGlobals());

describe("useFeaturedOdds", () => {
  it("shows — when the API is unreachable", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("offline")));
    const { result } = renderHook(() => useFeaturedOdds());
    await waitFor(() => expect(result.current.market).toBeNull());
    expect(result.current.yes).toBeNull();
  });

  it("picks the largest open pool and moves on odds_updated", async () => {
    const markets = [
      { market_id: "a", question: "A?", status: "OPEN", resolution_type: "POOL_THRESHOLD", close_height: 1, pool_total_zat: 100,
        odds: [{ outcome_index: 0, label: "YES", probability: 0.5 }, { outcome_index: 1, label: "NO", probability: 0.5 }] },
      { market_id: "b", question: "B?", status: "OPEN", resolution_type: "POOL_THRESHOLD", close_height: 1, pool_total_zat: 1_240_000_000,
        odds: [{ outcome_index: 0, label: "YES", probability: 0.621 }, { outcome_index: 1, label: "NO", probability: 0.379 }] },
    ];
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ ok: true, json: async () => ({ markets }) }));
    vi.stubGlobal("EventSource", MockEventSource);
    const { result } = renderHook(() => useFeaturedOdds());
    await waitFor(() => expect(result.current.market?.market_id).toBe("b"));
    expect(MockEventSource.last?.url).toContain("/markets/b/stream");
    act(() =>
      MockEventSource.last!.emit("odds_updated", {
        pool_total_zat: 1_300_000_000,
        outcomes: [{ outcome_index: 0, probability: 0.7 }, { outcome_index: 1, probability: 0.3 }],
      }),
    );
    expect(result.current.yes).toBe(0.7);
    expect(result.current.poolZat).toBe(1_300_000_000);
  });
});
