"use client";
// Read-only public data; private actions stay on the coordinator host (docs/DESIGN.md).
import Link from "next/link";
import { useEffect, useState } from "react";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { CommitteePanel } from "./CommitteePanel";
import { SigningProgress } from "./Settlement";
import { fetchTip } from "@/lib/api";
import { operatorRows, type OperatorRow } from "@/lib/operator";

export function OperatorStatus({ initialRows }: { initialRows: OperatorRow[] | null }) {
  const [rows, setRows] = useState(initialRows);
  const [tip, setTip] = useState<number | null>(null);
  useEffect(() => {
    let alive = true, busy = false;
    const refresh = async () => {
      if (busy) return;
      busy = true;
      const [next, height] = await Promise.all([operatorRows(), fetchTip()]);
      if (alive) { setRows(next); setTip(height); }
      busy = false;
    };
    void refresh();
    const timer = setInterval(refresh, 5000);
    return () => { alive = false; clearInterval(timer); };
  }, []);
  return <section className="bettor-page">
    <p className="section-index">S-11 / PUBLIC STATUS</p>
    <h1 className="t-2 my-6">Operator status.</h1>
    <p className="t-4 muted">Read-only. This demo uses a trusted dealer. Signing approvals happen on the coordinator host.</p>
    {rows === null ? <p className="ledger-panel t-4" role="status">Public API unavailable.</p> : !rows.length ? <p className="ledger-panel t-4">No markets published.</p> : rows.map(({ market, payout, committee }) => <article className="ledger-panel" key={market.market_id}>
      <div className="ledger-title"><h2 className="t-3"><Link className="text-link" href={`/markets/${encodeURIComponent(market.market_id)}`}>{market.question}</Link></h2><StatusBadge status={market.status} /></div>
      <p className="num t-5 muted mt-3">{market.market_id}</p>
      {committee && <CommitteePanel definition={committee.definition} resolution={committee.resolution} tip={tip} />}
      {payout ? <div className="mt-4"><StatusBadge status={payout.status} /><p className="ledger-hash num t-5 mt-3">plan_hash: {payout.plan_hash}</p><SigningProgress signing={payout.signing} /></div> : <p className="t-4 muted mt-4">No public payout plan published.</p>}
    </article>)}
    <section className="ledger-panel"><h2 className="t-3">On the coordinator host</h2><p className="t-4 muted mt-3">Run private commands locally. This page sends no operator requests.</p><pre className="ledger-code num t-5">{"frosted-operator resolution propose <market_id> --outcome 0 --evidence-url <url> --member member-1\nfrosted-operator resolution confirm <market_id> --outcome 0 --evidence-url <url> --member member-2\nfrosted-operator resolution withdraw <market_id> --member member-3\nfrosted-operator payout plan <market_id>\nfrosted-operator payout sign <market_id> --plan-hash <hash>\nfrosted-operator payout status <market_id>"}</pre></section>
  </section>;
}
