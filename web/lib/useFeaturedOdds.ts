"use client";

import { useEffect, useState } from "react";

import { featuredMarket, fetchMarkets, streamUrl, type MarketSummary } from "@/lib/api";

export type FeaturedOdds = {
  market: MarketSummary | null;
  yes: number | null;
  no: number | null;
  poolZat: number | null;
};

const EMPTY: FeaturedOdds = { market: null, yes: null, no: null, poolZat: null };

type OddsEvent = {
  pool_total_zat?: number;
  outcomes?: { outcome_index: number; probability: number | null }[];
};

/** Featured market odds from GET /v1/markets, updated on SSE `odds_updated`. "—" when unreachable. */
export function useFeaturedOdds(): FeaturedOdds {
  const [state, setState] = useState<FeaturedOdds>(EMPTY);
  useEffect(() => {
    let source: EventSource | null = null;
    let alive = true;
    (async () => {
      const market = featuredMarket(await fetchMarkets());
      if (!alive || !market) return;
      const prob = (i: number) => market.odds.find((o) => o.outcome_index === i)?.probability ?? null;
      setState({ market, yes: prob(0), no: prob(1), poolZat: market.pool_total_zat });
      source = new EventSource(streamUrl(market.market_id));
      source.addEventListener("odds_updated", (e) => {
        try {
          const data = JSON.parse((e as MessageEvent).data) as OddsEvent;
          const p = (i: number) => data.outcomes?.find((o) => o.outcome_index === i)?.probability ?? null;
          setState((s) => ({ ...s, yes: p(0), no: p(1), poolZat: data.pool_total_zat ?? s.poolZat }));
        } catch {
          /* ignore malformed events */
        }
      });
    })();
    return () => {
      alive = false;
      source?.close();
    };
  }, []);
  return state;
}
