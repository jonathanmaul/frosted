// Persona (docs/PROJECT_SPEC_FROSTED.md F-09, ADR-0002). `K_root` exists in plain form only in memory:
// at creation (backup step), when the user decrypts it for a tagged bet, and at Thaw.
import { getJson } from "./api";
import { database, listReceipts, type PersonaRecord } from "./receipts";

export const PBKDF2_ITERATIONS = 600_000;
export const MIN_PASSPHRASE = 12;
export const ALIAS_PATTERN = /^[A-Za-z0-9_.-]{1,32}$/;

export type PersonaInfo = { persona_id: string; commitment: string; registered_height: number; status: "REGISTERED" | "THAWED"; alias: string | null; thawed_at: number | null; k_root_b64?: string };
export type ThawBet = {
  market_id: string; note_ref: string; outcome_index: number; amount_zat: number; block_height: number;
  result: "PENDING" | "WON" | "LOST" | "REFUNDED" | "OTHER"; payout_zat: number | null;
  counted_in_reputation: boolean; exclusion: null | "PRE_REGISTRATION" | "DUPLICATE_TAG";
};
export type ThawReport = {
  persona_id: string; alias: string; registered_height: number; bets: ThawBet[];
  stats: { settled: number; won: number; win_rate: number | null; roi: number | null };
};
export type LeaderboardEntry = { persona_id: string; alias: string | null; registered_height: number; settled: number; won: number; win_rate: number | null; roi: number | null };
export type Registry = { uivk: string; address: string; birthday_height: number };

export const BADGE_MIN_BETS = 10;
export const EXCLUSION_LABELS = { PRE_REGISTRATION: "Before registration", DUPLICATE_TAG: "Duplicate tag" } as const;
export const RESULT_LABELS: Record<ThawBet["result"], string> = { PENDING: "Pending", WON: "Won", LOST: "Lost", REFUNDED: "Refunded", OTHER: "Not settled" };

export const fetchRegistry = () => getJson<Registry | null>("/personas/registry");
export const fetchPersona = (id: string) => getJson<PersonaInfo>(`/personas/${encodeURIComponent(id)}`);
export const fetchThawReport = (id: string) => getJson<ThawReport>(`/personas/${encodeURIComponent(id)}/report`);
export async function fetchLeaderboard(): Promise<LeaderboardEntry[] | null> {
  const body = await getJson<{ entries: LeaderboardEntry[] }>("/leaderboard");
  return body ? body.entries : null;
}

const bytesOf = (hex: string) => Uint8Array.from(hex.match(/../g) ?? [], (b) => parseInt(b, 16));
const hexOf = (bytes: Uint8Array) => Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
const b64 = (bytes: Uint8Array) => btoa(String.fromCharCode(...bytes));
const unb64 = (text: string) => Uint8Array.from(atob(text), (c) => c.charCodeAt(0));
/** base64url without padding, as `POST /v1/personas/{id}/thaw` expects. */
export const rootToBase64Url = (hex: string) => b64(bytesOf(hex)).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
export const isRootHex = (value: string) => /^[a-f0-9]{64}$/.test(value);

async function deriveKey(passphrase: string, salt: Uint8Array, iterations: number) {
  const material = await crypto.subtle.importKey("raw", new TextEncoder().encode(passphrase), "PBKDF2", false, ["deriveKey"]);
  return crypto.subtle.deriveKey({ name: "PBKDF2", hash: "SHA-256", salt: salt as BufferSource, iterations }, material, { name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
}

/** AES-GCM under a PBKDF2 key. The Persona id is authenticated, so a record cannot be swapped onto another id. */
export async function encryptRoot(kRootHex: string, passphrase: string, personaId: string, iterations = PBKDF2_ITERATIONS): Promise<PersonaRecord["enc"]> {
  if (!isRootHex(kRootHex)) throw new Error("Persona root must be 32 bytes of hex.");
  if (passphrase.length < MIN_PASSPHRASE) throw new Error(`Passphrase needs at least ${MIN_PASSPHRASE} characters.`);
  const salt = crypto.getRandomValues(new Uint8Array(16)), iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(passphrase, salt, iterations);
  const ct = new Uint8Array(await crypto.subtle.encrypt({ name: "AES-GCM", iv, additionalData: new TextEncoder().encode(personaId) }, key, bytesOf(kRootHex)));
  return { v: 1, kdf: "PBKDF2-SHA256", iterations, salt: b64(salt), iv: b64(iv), ct: b64(ct) };
}
export async function decryptRoot(record: PersonaRecord, passphrase: string): Promise<string> {
  try {
    const { enc } = record;
    const key = await deriveKey(passphrase, unb64(enc.salt), enc.iterations);
    const plain = await crypto.subtle.decrypt({ name: "AES-GCM", iv: unb64(enc.iv) as BufferSource, additionalData: new TextEncoder().encode(record.persona_id) }, key, unb64(enc.ct) as BufferSource);
    return hexOf(new Uint8Array(plain));
  } catch { throw new Error("Wrong passphrase, or the stored Persona is damaged."); }
}

export async function savePersona(record: PersonaRecord) {
  const db = await database();
  try { await db.put("personas", record); } finally { db.close(); }
}
/** One active Persona per browser. */
export async function getPersona(): Promise<PersonaRecord | null> {
  const db = await database();
  try { return (await db.getAll("personas")).sort((a, b) => b.created_at - a.created_at)[0] ?? null; } finally { db.close(); }
}
/** Local count only: receipts this device tagged with a Persona (S-06). */
export async function taggedBetCount(): Promise<number> {
  return (await listReceipts()).filter((r) => r.tagged).length;
}

export function validateAlias(alias: string, confirmation: string): string | null {
  if (!ALIAS_PATTERN.test(alias)) return "Alias: 1 to 32 letters, digits, '.', '_' or '-'.";
  if (alias !== confirmation) return "Type the alias exactly to confirm.";
  return null;
}

export const formatRoi = (roi: number | null | undefined) => roi === null || roi === undefined ? "—" : `${roi > 0 ? "+" : ""}${Math.round(roi * 100)}%`;
export const hasBadge = (settled: number) => settled >= BADGE_MIN_BETS;

export type MarketLabel = { question: string; outcomes: { outcome_index: number; label: string }[] };
export async function fetchMarketLabels(ids: string[]): Promise<Record<string, MarketLabel>> {
  const out: Record<string, MarketLabel> = {};
  await Promise.all([...new Set(ids)].map(async (id) => {
    const market = await getJson<{ market_definition: MarketLabel }>(`/markets/${encodeURIComponent(id)}`);
    if (market) out[id] = { question: market.market_definition.question, outcomes: market.market_definition.outcomes };
  }));
  return out;
}
