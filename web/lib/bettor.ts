import { getJson } from "./api";

export type MarketDefinition = {
  version: number; params_version: number; network: string; market_id: string; question: string;
  outcomes: { outcome_index: number; label: string; address: string }[];
  resolution_type: string; resolution_params: Record<string, unknown>;
  birthday_height: number; close_height: number; resolve_height: number; fee_bps: number;
  min_bet_zat: number; refund_fee_zat: number; min_confirmations: number; sweep_threshold_zat: number;
  announce_timeout_blocks: number; uivk: string; envelope_pk: string; treasury_address: string;
};
export type Market = { market_definition: MarketDefinition; rules_hash: string; status: string; announce_height: number | null; announce_txid?: string | null };
export type Odds = { at_height: number; pool_total_zat: number; pending_bets: number; outcomes: { outcome_index: number; probability: number | null; pool_zat: number; bets: number }[] };
export type PublicBet = { note_ref: string; bet_fingerprint: string | null; outcome_index: number | null; amount_zat: number; block_height: number; status: string; tagged: boolean };
export type PlanItem = { note_ref: string; bet_fingerprint: string | null; kind: string; amount_zat: number; receiver_commitment: string | null };
export type Resolution = {
  market_id: string; status: string; winning_outcome_index: number | null;
  evidence: { source: string; height: number; block_hash: string; pool?: string; chain_value_zat?: number;
    reported_upgrades?: { name: string; activation_height: number }[] } | null;
  votes: { member_id: string; action: string; outcome_index: number | null; evidence_url: string | null; created_at: number }[];
};
export type Payout = {
  plan_hash: string; status: string;
  plan: { market_id: string; plan_kind: string; plan_cutoff_height: number; result: number | "VOID";
    operator_fee_zat: number; items: PlanItem[] };
  summary: { winners: number; refunds: number; unclaimable: number; swept: number; uneconomic_dust: number;
    operator_fee_zat: number; tx_fee_zat: number; distributed_zat: number };
  signing: { sessions_done: number; sessions_total: number; threshold: number };
  txs: { txid: string; status: string; confirmed_height: number | null }[];
};
export const marketPath = (id: string) => `/markets/${encodeURIComponent(id)}`;
export const fetchMarket = (id: string) => getJson<Market>(marketPath(id));
export const fetchOdds = (id: string) => getJson<Odds>(`${marketPath(id)}/odds`);
export const fetchPayout = (id: string) => getJson<Payout>(`${marketPath(id)}/payout`);
export const fetchResolution = (id: string) => getJson<Resolution>(`${marketPath(id)}/resolution`);
export async function fetchBets(id: string): Promise<PublicBet[] | null> {
  const page = await getJson<{ bets: PublicBet[] }>(`${marketPath(id)}/bets?limit=500`);
  return page?.bets ?? null;
}

/** Decimal ZEC input to zatoshi, without floating point rounding. */
export function parseAmount(value: string): number {
  if (!/^\d+(\.\d{1,8})?$/.test(value)) throw new Error("Enter ZEC with at most 8 decimal places.");
  const [whole, fraction = ""] = value.split(".");
  const zat = BigInt(whole) * 100_000_000n + BigInt(fraction.padEnd(8, "0"));
  if (zat <= 0n || zat > 2_100_000_000_000_000n) throw new Error("Amount is outside the Zcash money range.");
  return Number(zat);
}
export function exactZec(zat: number): string {
  return `${Math.floor(zat / 100_000_000)}.${String(zat % 100_000_000).padStart(8, "0")}`;
}
