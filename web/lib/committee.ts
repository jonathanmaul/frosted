// Committee-Resolved markets (docs/PROJECT_SPEC_FROSTED.md F-11, UF-4). Read-only: votes are cast
// with `frosted-operator` on the coordinator host, never from the browser.
import type { MarketDefinition, Resolution } from "./bettor";

export type CommitteeParams = { criteria: string; evidence_source: string; dispute_window_blocks: number; void_after_blocks: number };

export function committeeParams(def: MarketDefinition): CommitteeParams | null {
  if (def.resolution_type !== "COMMITTEE") return null;
  const p = def.resolution_params as Partial<CommitteeParams>;
  if (typeof p.dispute_window_blocks !== "number" || typeof p.void_after_blocks !== "number") return null;
  return { criteria: String(p.criteria ?? ""), evidence_source: String(p.evidence_source ?? ""), dispute_window_blocks: p.dispute_window_blocks, void_after_blocks: p.void_after_blocks };
}

export type Countdown =
  | { kind: "dispute"; endsAt: number; blocksLeft: number | null }
  | { kind: "void"; endsAt: number; blocksLeft: number | null }
  | { kind: "none" };

/**
 * PROPOSED: the dispute window runs from the block recorded in the evidence. Otherwise, until the
 * market is decided, `void_after_blocks` runs from `resolve_height`. `tip` null keeps the count unknown.
 */
export function committeeCountdown(def: MarketDefinition, resolution: Resolution | null, tip: number | null): Countdown {
  const params = committeeParams(def);
  if (!params || resolution?.status === "RESOLVED" || resolution?.status === "VOID") return { kind: "none" };
  const left = (endsAt: number) => (tip === null ? null : endsAt - tip);
  if (resolution?.status === "PROPOSED" && resolution.evidence) {
    const endsAt = resolution.evidence.height + params.dispute_window_blocks;
    return { kind: "dispute", endsAt, blocksLeft: left(endsAt) };
  }
  const endsAt = def.resolve_height + params.void_after_blocks;
  return { kind: "void", endsAt, blocksLeft: left(endsAt) };
}

export const VOTE_LABELS: Record<string, string> = { PROPOSE: "Proposed", CONFIRM: "Confirmed", WITHDRAW: "Withdrawn" };

/** Evidence URLs come from the coordinator; only plain web links are made clickable. */
export const isWebUrl = (url: string) => /^https?:\/\/[^\s]+$/.test(url);

export function countdownText(c: Countdown): string {
  if (c.kind === "none") return "";
  const blocks = c.blocksLeft === null ? "— blocks left" : c.blocksLeft > 0 ? `${c.blocksLeft.toLocaleString("en-US")} ${c.blocksLeft === 1 ? "block" : "blocks"} left` : "deadline reached";
  return c.kind === "dispute"
    ? `Dispute window ends at block ${c.endsAt.toLocaleString("en-US")}: ${blocks}. Any member can still withdraw.`
    : `Undecided markets are voided at block ${c.endsAt.toLocaleString("en-US")}: ${blocks}. Every counted bet is then refunded.`;
}
