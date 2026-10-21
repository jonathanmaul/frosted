// Minimal public API client (docs/PROJECT_SPEC_FROSTED.md §22). Every call tolerates the API being
// unreachable: callers render "—" instead of invented values.
export const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080/v1";

export type OutcomeOdds = { outcome_index: number; label: string; probability: number | null };
export type MarketSummary = {
  market_id: string;
  question: string;
  status: string;
  resolution_type: string;
  close_height: number;
  pool_total_zat: number;
  odds: OutcomeOdds[];
};

async function getJson<T>(path: string): Promise<T | null> {
  try {
    const res = await fetch(`${API_URL}${path}`, { cache: "no-store" });
    if (!res.ok) return null;
    return (await res.json()) as T;
  } catch {
    return null;
  }
}

export async function fetchMarkets(): Promise<MarketSummary[] | null> {
  const body = await getJson<{ markets: MarketSummary[] }>("/markets");
  return body ? body.markets : null;
}

export async function fetchTip(): Promise<number | null> {
  const body = await getJson<{ height: number }>("/chain/tip");
  return body ? body.height : null;
}

/** Featured market: the OPEN market with the largest pool (docs/DESIGN.md §6). */
export function featuredMarket(markets: MarketSummary[] | null): MarketSummary | null {
  if (!markets) return null;
  const open = markets.filter((m) => m.status === "OPEN");
  if (open.length === 0) return null;
  return open.reduce((a, b) => (b.pool_total_zat > a.pool_total_zat ? b : a));
}

export function streamUrl(marketId: string): string {
  return `${API_URL}/markets/${marketId}/stream`;
}

/** Formats zatoshi as ZEC with two decimals, without floating point. */
export function formatZec(zat: number | null | undefined): string {
  if (zat === null || zat === undefined) return "—";
  const whole = Math.floor(zat / 100_000_000);
  const cents = Math.floor((zat % 100_000_000) / 1_000_000);
  return `${whole.toLocaleString("en-US")}.${String(cents).padStart(2, "0")}`;
}

export function formatPercent(p: number | null | undefined): string {
  if (p === null || p === undefined) return "—";
  return `${Math.round(p * 100)}%`;
}
