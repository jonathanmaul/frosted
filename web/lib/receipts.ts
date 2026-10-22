import { openDB, type DBSchema } from "idb";
import type { PlanItem, PublicBet } from "./bettor";

export type BetReceipt = {
  bet_fingerprint: string; market_id: string; outcome_index: number; amount_zat: number;
  payout_ua: string; bet_salt: string; env_esk: string; tagged: boolean; note_ref: string | null;
  created_at: number; last_status: string; amount_mismatch: boolean;
};
interface FrostedDB extends DBSchema {
  bet_receipts: { key: string; value: BetReceipt };
}
function database() {
  return openDB<FrostedDB>("frosted", 1, { upgrade(db) {
    if (!db.objectStoreNames.contains("bet_receipts")) db.createObjectStore("bet_receipts", { keyPath: "bet_fingerprint" });
  } });
}
export async function saveReceipt(receipt: BetReceipt) {
  const db = await database();
  try { await db.put("bet_receipts", receipt); } finally { db.close(); }
}
export async function listReceipts() {
  const db = await database();
  try { return (await db.getAll("bet_receipts")).sort((a, b) => b.created_at - a.created_at); } finally { db.close(); }
}
export function parseReceipts(json: string): BetReceipt[] {
  if (json.length > 5_000_000) throw new Error("Receipt file exceeds 5 MB.");
  const input: unknown = JSON.parse(json);
  if (!Array.isArray(input) || input.length > 10000) throw new Error("Expected an array of receipts (maximum 10,000).");
  const hex = (v: unknown, n: number) => typeof v === "string" && new RegExp(`^[a-f0-9]{${n}}$`).test(v);
  const integer = (v: unknown) => Number.isSafeInteger(v) && Number(v) >= 0;
  return input.map((r: BetReceipt) => {
    if (!r || !hex(r.bet_fingerprint, 16) || !hex(r.market_id, 32) || !hex(r.bet_salt, 16) || !hex(r.env_esk, 64)
      || !integer(r.amount_zat) || r.amount_zat <= 0 || r.amount_zat > 2_100_000_000_000_000 || !integer(r.outcome_index) || r.outcome_index > 255
      || typeof r.payout_ua !== "string" || r.payout_ua.length > 2048 || !/^u(?:test|regtest)?1[0-9a-z]+$/.test(r.payout_ua)
      || !integer(r.created_at) || !Number.isFinite(new Date(r.created_at).getTime()) || typeof r.tagged !== "boolean") throw new Error("Invalid receipt. Nothing was imported.");
    // Public matching state is recomputed; imported JSON never overwrites trusted observations.
    return { bet_fingerprint: r.bet_fingerprint, market_id: r.market_id, outcome_index: r.outcome_index, amount_zat: r.amount_zat,
      payout_ua: r.payout_ua, bet_salt: r.bet_salt, env_esk: r.env_esk, tagged: r.tagged, created_at: r.created_at,
      note_ref: null, last_status: "WAITING", amount_mismatch: false };
  });
}
export async function importReceipts(json: string) {
  const receipts = parseReceipts(json);
  const db = await database();
  try {
    const tx = db.transaction("bet_receipts", "readwrite");
    for (const receipt of receipts) {
      const existing = await tx.store.get(receipt.bet_fingerprint);
      if (existing && ["market_id", "outcome_index", "amount_zat", "payout_ua", "bet_salt", "env_esk", "tagged"].some(k => existing[k as keyof BetReceipt] !== receipt[k as keyof BetReceipt])) {
        tx.abort(); await tx.done.catch(() => {}); throw new Error("Conflicting receipt fingerprint. Nothing was imported.");
      }
      if (!existing) await tx.store.add(receipt);
    }
    await tx.done;
  } finally { db.close(); }
}
export function matchReceipt(receipt: BetReceipt, bets: PublicBet[]): BetReceipt {
  const matches = bets.filter(b => b.bet_fingerprint === receipt.bet_fingerprint);
  if (matches.length > 1) return { ...receipt, last_status: "AMBIGUOUS", note_ref: null, amount_mismatch: false };
  const bet = matches[0];
  return { ...receipt, note_ref: bet?.note_ref ?? null, last_status: bet?.status ?? (bets.length >= 500 ? "UNAVAILABLE" : "WAITING"), amount_mismatch: !!bet && bet.amount_zat !== receipt.amount_zat };
}
export function checkPlan(receipt: BetReceipt, items: PlanItem[], commitment: string): string {
  const item = items.find(i => i.note_ref === receipt.note_ref);
  if (!item || (item.bet_fingerprint !== null && item.bet_fingerprint !== receipt.bet_fingerprint)) return "Mismatch: no matching plan item for this note.";
  if (["SWEEP", "UNCLAIMABLE", "UNECONOMIC_DUST"].includes(item.kind) && item.receiver_commitment === null) return `${item.kind}: ${item.amount_zat} zat. No payout receiver in this item.`;
  if (item.receiver_commitment !== commitment) return "Mismatch: receiver commitment does not match this note.";
  return `${item.kind}: ${item.amount_zat} zat. Receiver commitment matches.`;
}
export function downloadJson(value: unknown, filename: string) {
  const url = URL.createObjectURL(new Blob([JSON.stringify(value, null, 2)], { type: "application/json" }));
  const a = document.createElement("a"); a.href = url; a.download = filename; a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
