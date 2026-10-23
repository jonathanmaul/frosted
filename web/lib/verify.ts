// S-10 (docs/PROJECT_SPEC_FROSTED.md F-12). The server-side check is a convenience: the CLI on the
// reader's own node is the audit.
import { API_URL } from "./api";

export type VerifyStatus = "PASS" | "FAIL" | "SKIP" | "INFO";
export type VerifyCheck = { id: string; name: string; status: VerifyStatus; detail: string; diff?: { verifier: unknown; api: unknown } };
export type VerifyReport = { market_id: string; command: string; node_tip: number; api_height: number | null; checks: VerifyCheck[]; ok: boolean; trust: string };
export type VerifyResult = { ok: true; report: VerifyReport } | { ok: false; message: string };

export const DEFAULT_NODE = "http://127.0.0.1:8232";
export const SERVER_CHECK_TIMEOUT_MS = 130_000;

/** The exact commands a reader runs against their own node. */
export function verifyCommands(marketId: string, node = DEFAULT_NODE): { label: string; command: string }[] {
  return [
    { label: "Everything for this market", command: `frosted-verify all ${marketId} --node ${node}` },
    { label: "Pools only", command: `frosted-verify pools ${marketId} --node ${node}` },
    { label: "Chain-Verified outcome only", command: `frosted-verify resolution ${marketId} --node ${node}` },
  ];
}

export async function fetchServerCheck(marketId: string): Promise<VerifyResult> {
  try {
    const res = await fetch(`${API_URL}/markets/${encodeURIComponent(marketId)}/verify`, { cache: "no-store", credentials: "omit", signal: AbortSignal.timeout(SERVER_CHECK_TIMEOUT_MS) });
    const body = await res.json().catch(() => null);
    if (res.ok) return { ok: true, report: body as VerifyReport };
    return { ok: false, message: body?.error?.message ?? "The server-side check failed." };
  } catch {
    return { ok: false, message: "The public API is unreachable, or the check timed out." };
  }
}

export const formatDiffValue = (value: unknown): string => typeof value === "string" ? value : JSON.stringify(value);
