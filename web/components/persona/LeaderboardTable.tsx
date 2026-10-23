// S-09. Only thawed Personas, ranked by ROI as published in their Thaw Reports.
import Link from "next/link";
import { MonoNumber } from "@/components/ui/MonoNumber";
import { formatPercent } from "@/lib/api";
import { BADGE_MIN_BETS, formatRoi, hasBadge, type LeaderboardEntry } from "@/lib/persona";

export function LeaderboardTable({ entries }: { entries: LeaderboardEntry[] | null }) {
  if (entries === null) return <p role="status" className="t-4 muted">The public API is unreachable. Rankings are —.</p>;
  return <div>
    {entries.length === 0 ? <p className="t-4 muted">No thawed Personas yet.</p> :
      <div className="table-scroll"><table className="ledger-table num t-5"><caption className="sr-only">Thawed Personas ranked by return on investment.</caption>
        <thead><tr><th scope="col">Rank</th><th scope="col">Persona</th><th scope="col">Settled bets</th><th scope="col">Win rate</th><th scope="col">ROI</th><th scope="col">Registered</th></tr></thead>
        <tbody>{entries.map((e, i) => <tr key={e.persona_id}>
          <td><MonoNumber>{i + 1}</MonoNumber></td>
          <td><Link className="text-link" href={`/p/${e.persona_id}`}>{e.alias ?? e.persona_id.slice(0, 8)}</Link></td>
          <td><MonoNumber>{e.settled}</MonoNumber>{hasBadge(e.settled) && <span className="badge-10"> {BADGE_MIN_BETS}+</span>}</td>
          <td>{formatPercent(e.win_rate)}</td><td>{formatRoi(e.roi)}</td>
          <td>blk <MonoNumber>{e.registered_height.toLocaleString("en-US")}</MonoNumber></td></tr>)}</tbody></table></div>}
    <p className="t-5 muted mt-6">Personas with fewer than {BADGE_MIN_BETS} settled bets are shown without the {BADGE_MIN_BETS}+ badge. One person may control several Personas.</p>
  </div>;
}
