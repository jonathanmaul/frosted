// Read-only committee status for S-05 and S-11 (docs/DESIGN.md: flat panel, text carries the meaning).
import { MonoNumber } from "@/components/ui/MonoNumber";
import type { MarketDefinition, Resolution } from "@/lib/bettor";
import { committeeCountdown, committeeParams, countdownText, isWebUrl, VOTE_LABELS } from "@/lib/committee";

export function CommitteePanel({ definition, resolution, tip }: { definition: MarketDefinition; resolution: Resolution | null; tip: number | null }) {
  const params = committeeParams(definition);
  if (!params) return null;
  const countdown = committeeCountdown(definition, resolution, tip);
  const label = (i: number | null) => definition.outcomes.find((o) => o.outcome_index === i)?.label ?? "—";
  return <div className="committee-panel mt-6" aria-labelledby={`committee-${definition.market_id}`}>
    <h3 id={`committee-${definition.market_id}`} className="t-3">Committee decision</h3>
    <p className="t-4 muted">{params.criteria}</p>
    <p className="t-5 muted mt-2">Evidence source: {params.evidence_source}</p>
    {countdown.kind !== "none" && <p role="status" className="num t-4 mt-4">{countdownText(countdown)}</p>}
    {resolution?.votes.length ? <ul className="verify-list">{resolution.votes.map((v, i) => <li key={`${v.member_id}-${v.action}-${i}`} className="verify-row">
      <p className="num t-5">{v.member_id} · {VOTE_LABELS[v.action] ?? v.action} · {label(v.outcome_index)}</p>
      {v.evidence_url && <p className="t-5 muted">{isWebUrl(v.evidence_url) ? <a className="text-link" href={v.evidence_url} rel="noopener noreferrer" target="_blank">{v.evidence_url}</a> : v.evidence_url}</p>}
    </li>)}</ul> : <p className="t-4 muted mt-4">No proposal yet. Two of three members must agree on an outcome and its evidence.</p>}
    <p className="t-5 muted mt-4">Votes are cast on the coordinator host with <MonoNumber>frosted-operator resolution</MonoNumber>. In this deployment one person holds all three member keys, so the dispute window is a simulation of a real committee, not a check by independent parties.</p>
  </div>;
}
