// Minimal public API client (docs/PROJECT_SPEC_FROSTED.md §22). Every call tolerates the API being
// unreachable: callers render "—" instead of invented values.
import snapshot from "./snapshot.json";

export const API_URL = process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8080/v1";

/**
 * Snapshot mode (NEXT_PUBLIC_SNAPSHOT_MODE=true, spec M12.5): a deployment without a live backend serves recorded responses of
 * the real testnet API (lib/snapshot.json, written by scripts/snapshot.mjs). Nothing is invented; live-only
 * features (SSE, Thaw, the server-side check) are off.
 */
export const SNAPSHOT = process.env.NEXT_PUBLIC_SNAPSHOT_MODE === "true";
const RECORDED = snapshot.responses as Record<string, unknown>;

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

export async function getJson<T>(path: string): Promise<T | null> {
  if (SNAPSHOT) return (RECORDED[path] ?? null) as T | null;
  try {
    const res = await fetch(`${API_URL}${path}`, { cache: "no-store", credentials: "omit", signal: AbortSignal.timeout(5000) });
    if (!res.ok) return null;
    return (await res.json()) as T;
  } catch {
    return null;
  }
}

export type PostResult<T> = { ok: true; data: T } | { ok: false; code: string; message: string };

/** The only write the browser makes to the API (Thaw). Credential-free, JSON in and out. */
export async function postJson<T>(path: string, body: unknown): Promise<PostResult<T>> {
  if (SNAPSHOT) return { ok: false, code: "NETWORK", message: "This action needs the live backend." };
  try {
    const res = await fetch(`${API_URL}${path}`, { method: "POST", credentials: "omit", headers: { "content-type": "application/json" },
      body: JSON.stringify(body), signal: AbortSignal.timeout(30000) });
    const json = await res.json().catch(() => null);
    if (res.ok) return { ok: true, data: json as T };
    return { ok: false, code: json?.error?.code ?? "INTERNAL", message: json?.error?.message ?? "Request failed." };
  } catch {
    return { ok: false, code: "NETWORK", message: "The public API is unreachable." };
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
  // A snapshot has no open market once its recorded market settled; feature the largest recorded pool.
  const pool = open.length ? open : SNAPSHOT ? markets.filter((m) => m.pool_total_zat > 0) : [];
  if (pool.length === 0) return null;
  return pool.reduce((a, b) => (b.pool_total_zat > a.pool_total_zat ? b : a));
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
