"use client";
// Read-only public data; private actions stay on the coordinator host (docs/DESIGN.md).
import Link from "next/link";
import { useEffect, useState } from "react";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { SigningProgress } from "./Settlement";
import { fetchMarkets, type MarketSummary } from "@/lib/api";
import { fetchPayout, type Payout } from "@/lib/bettor";

export type OperatorRow = { market: MarketSummary; payout: Payout | null };
export async function operatorRows(): Promise<OperatorRow[] | null> {
  const markets = await fetchMarkets();
  if (!markets) return null;
  return Promise.all(markets.map(async market => ({ market, payout: await fetchPayout(market.market_id) })));
}

export function OperatorStatus({ initialRows }: { initialRows: OperatorRow[] | null }) {
  const [rows, setRows] = useState(initialRows);
  useEffect(() => {
    let alive = true, busy = false;
    const timer = setInterval(async () => {
      if (busy) return;
      busy = true;
      const next = await operatorRows();
      if (alive) setRows(next);
      busy = false;
    }, 5000);
    return () => { alive = false; clearInterval(timer); };
  }, []);
  return <section className="bettor-page">
    <p className="section-index">S-11 / PUBLIC STATUS</p>
    <h1 className="t-2 my-6">Operator status.</h1>
    <p className="t-4 muted">Read-only. This demo uses a trusted dealer. Signing approvals happen on the coordinator host.</p>
    {rows === null ? <p className="ledger-panel t-4" role="status">Public API unavailable.</p> : !rows.length ? <p className="ledger-panel t-4">No markets published.</p> : rows.map(({ market, payout }) => <article className="ledger-panel" key={market.market_id}>
      <div className="ledger-title"><h2 className="t-3"><Link className="text-link" href={`/markets/${encodeURIComponent(market.market_id)}`}>{market.question}</Link></h2><StatusBadge status={market.status} /></div>
      <p className="num t-5 muted mt-3">{market.market_id}</p>
      {payout ? <div className="mt-4"><StatusBadge status={payout.status} /><p className="ledger-hash num t-5 mt-3">plan_hash: {payout.plan_hash}</p><SigningProgress signing={payout.signing} /></div> : <p className="t-4 muted mt-4">No public payout plan published.</p>}
    </article>)}
    <section className="ledger-panel"><h2 className="t-3">On the coordinator host</h2><p className="t-4 muted mt-3">Run private commands locally. This page sends no operator requests.</p><pre className="ledger-code num t-5">{"frosted-operator payout plan <market_id>\nfrosted-operator payout sign <market_id> --plan-hash <hash>\nfrosted-operator payout status <market_id>"}</pre></section>
  </section>;
}
