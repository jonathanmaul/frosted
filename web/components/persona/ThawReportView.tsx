// Thaw Report as a ledger: every tagged bet, wins and losses, with exclusion reasons (S-07, S-08).
import Link from "next/link";
import { MonoNumber } from "@/components/ui/MonoNumber";
import { formatPercent, formatZec } from "@/lib/api";
import { EXCLUSION_LABELS, formatRoi, RESULT_LABELS, type MarketLabel, type ThawReport } from "@/lib/persona";

export function ThawReportView({ report, markets }: { report: ThawReport; markets: Record<string, MarketLabel> }) {
  const { stats } = report;
  return <div>
    <dl className="persona-stats num t-4">
      <div><dt className="t-5 muted">Settled bets</dt><dd>{stats.settled}</dd></div>
      <div><dt className="t-5 muted">Won</dt><dd>{stats.won}</dd></div>
      <div><dt className="t-5 muted">Win rate</dt><dd>{formatPercent(stats.win_rate)}</dd></div>
      <div><dt className="t-5 muted">ROI</dt><dd>{formatRoi(stats.roi)}</dd></div>
    </dl>
    {report.bets.length === 0 ? <p className="t-4 muted mt-6">No tagged bets matched this Persona.</p> :
      <div className="table-scroll"><table className="ledger-table num t-5"><caption className="sr-only">Every bet tagged by {report.alias}, including losses.</caption>
        <thead><tr><th scope="col">Market</th><th scope="col">Outcome</th><th scope="col">Stake (ZEC)</th><th scope="col">Block</th><th scope="col">Result</th><th scope="col">Payout (ZEC)</th><th scope="col">Record</th></tr></thead>
        <tbody>{report.bets.map((b) => { const market = markets[b.market_id]; return <tr key={b.note_ref}>
          <td><Link className="text-link" href={`/markets/${b.market_id}`}>{market?.question ?? b.market_id.slice(0, 8)}</Link></td>
          <td>{market?.outcomes.find((o) => o.outcome_index === b.outcome_index)?.label ?? `#${b.outcome_index}`}</td>
          <td><MonoNumber>{formatZec(b.amount_zat)}</MonoNumber></td>
          <td><MonoNumber>{b.block_height.toLocaleString("en-US")}</MonoNumber></td>
          <td>{RESULT_LABELS[b.result]}</td>
          <td><MonoNumber>{b.payout_zat === null ? "—" : formatZec(b.payout_zat)}</MonoNumber></td>
          <td>{b.exclusion ? `Excluded: ${EXCLUSION_LABELS[b.exclusion]}` : b.counted_in_reputation ? "Counted" : "Not counted yet"}</td></tr>; })}</tbody></table></div>}
    <p className="t-5 muted mt-4">Excluded and unsettled bets stay visible and do not enter win rate or ROI. Anyone can recompute this report from public data.</p>
  </div>;
}
