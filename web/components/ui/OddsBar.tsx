import { formatPercent } from "@/lib/api";

export type OddsSide = { label: string; probability: number | null };

/** Two-sided odds bar. Color is never alone: every side shows its label and percent (NFR-A2). */
export function OddsBar({ yes, no }: { yes: OddsSide; no: OddsSide }) {
  const known = yes.probability !== null && no.probability !== null;
  const yesShare = known ? Math.round((yes.probability ?? 0) * 1000) / 10 : 50;
  return (
    <div className="space-y-2">
      <div className="flex justify-between t-5">
        <span>
          <span className="text-[var(--yes)]">{yes.label}</span> <span className="num">{formatPercent(yes.probability)}</span>
        </span>
        <span>
          <span className="num">{formatPercent(no.probability)}</span> <span className="text-[var(--no)]">{no.label}</span>
        </span>
      </div>
      <div
        className="flex h-2 overflow-hidden rounded-[var(--radius-1)] border border-[var(--border)]"
        role="img"
        aria-label={`${yes.label} ${formatPercent(yes.probability)}, ${no.label} ${formatPercent(no.probability)}`}
      >
        <div className={known ? "bg-[var(--yes)]" : "bg-[var(--surface)]"} style={{ width: `${yesShare}%` }} />
        <div className={known ? "bg-[var(--no)]" : "bg-[var(--surface)]"} style={{ width: `${100 - yesShare}%` }} />
      </div>
    </div>
  );
}
