"use client";
// docs/DESIGN.md: flat ledger rows and existing base components.
import { useEffect, useState } from "react";
import { ButtonLink } from "@/components/ui/Button";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { MonoNumber } from "@/components/ui/MonoNumber";
import { fetchMarkets, fetchTip, formatPercent, formatZec, type MarketSummary } from "@/lib/api";

export function Markets({ initialMarkets, initialTip }: { initialMarkets: MarketSummary[] | null; initialTip: number | null }) {
  const [markets, setMarkets] = useState(initialMarkets);
  const [tip, setTip] = useState(initialTip);
  const [status, setStatus] = useState("");
  const [kind, setKind] = useState("");
  useEffect(() => { let alive = true; const refresh = async () => { const [m, t] = await Promise.all([fetchMarkets(), fetchTip()]); if (alive) { setMarkets(m); setTip(t); } }; const timer = setInterval(refresh, 15000); return () => { alive = false; clearInterval(timer); }; }, []);
  const filtered = markets?.filter(m => (!status || status === m.status) && (!kind || (m.resolution_type === "COMMITTEE" ? "committee" : "chain_verified") === kind));
  return <div className="market-list"><div className="ledger-filters"><label htmlFor="market-status">Status<select id="market-status" className="ledger-input" value={status} onChange={e => setStatus(e.target.value)}><option value="">All statuses</option>{[...new Set(["OPEN", "ANNOUNCING", "CLOSED", "AWAITING_RESOLUTION", "PROPOSED", "RESOLVED", "VOID", "PAYING", "SETTLED", ...(markets?.map(m => m.status) ?? [])])].map(s => <option key={s}>{s}</option>)}</select></label><label htmlFor="market-kind">Kind<select id="market-kind" className="ledger-input" value={kind} onChange={e => setKind(e.target.value)}><option value="">All kinds</option><option value="chain_verified">Chain verified</option><option value="committee">Committee</option></select></label></div>
    {filtered?.map(m => <article className="market-row" key={m.market_id}><div><StatusBadge status={m.status} /><h3 className="t-3 mt-4">{m.question}</h3><p className="t-5 muted">{m.resolution_type === "COMMITTEE" ? "Committee" : "Chain verified"} · <MonoNumber>{tip === null ? "—" : Math.max(0, m.close_height - tip).toLocaleString("en-US")}</MonoNumber> blocks remaining</p></div><div className="market-row-odds">{m.odds.map(o => <p className="t-4" key={o.outcome_index}>{o.label} <MonoNumber>{formatPercent(o.probability)}</MonoNumber></p>)}<p className="t-5 muted"><MonoNumber>{formatZec(m.pool_total_zat)} ZEC</MonoNumber> in the pool</p></div><ButtonLink href={`/markets/${encodeURIComponent(m.market_id)}`} variant="ghost">View market</ButtonLink></article>)}
    {!markets && <p className="ledger-panel t-4" role="status">Markets unavailable. Retry when the public API is reachable.</p>}{markets && !filtered?.length && <p className="ledger-panel t-4" role="status">No markets match these filters.</p>}
  </div>;
}
