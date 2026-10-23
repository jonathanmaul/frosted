// Rows for the read-only operator status (S-11). Shared by the server page and the live client.
import { fetchMarkets, type MarketSummary } from "./api";
import { fetchMarket, fetchPayout, fetchResolution, type MarketDefinition, type Payout, type Resolution } from "./bettor";

export type OperatorRow = { market: MarketSummary; payout: Payout | null; committee?: { definition: MarketDefinition; resolution: Resolution | null } | null };
export async function operatorRows(): Promise<OperatorRow[] | null> {
  const markets = await fetchMarkets();
  if (!markets) return null;
  return Promise.all(markets.map(async market => {
    const payout = await fetchPayout(market.market_id);
    if (market.resolution_type !== "COMMITTEE") return { market, payout, committee: null };
    const [full, resolution] = await Promise.all([fetchMarket(market.market_id), fetchResolution(market.market_id)]);
    return { market, payout, committee: full ? { definition: full.market_definition, resolution } : null };
  }));
}
