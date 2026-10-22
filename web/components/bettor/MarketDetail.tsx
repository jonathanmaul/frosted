"use client";
import Link from "next/link";
import { useEffect, useState } from "react";
import { BetBuilder } from "./BetBuilder";
import { OddsBar } from "@/components/ui/OddsBar";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { MonoNumber } from "@/components/ui/MonoNumber";
import { streamUrl, formatPercent, formatZec } from "@/lib/api";
import { fetchMarket, fetchOdds, fetchBets, type Market, type Odds, type PublicBet } from "@/lib/bettor";

export function MarketDetail({ initialMarket, initialOdds, initialBets }: { initialMarket: Market; initialOdds: Odds | null; initialBets: PublicBet[] | null }) {
  const [market, setMarket] = useState(initialMarket);
  const [odds, setOdds] = useState(initialOdds);
  const [bets, setBets] = useState(initialBets);
  const [connected, setConnected] = useState(false);
  const def = market.market_definition;
  useEffect(() => {
    let alive = true, refreshing = false, again = false;
    const refresh = async () => {
      if (refreshing) { again = true; return; }
      refreshing = true;
      do {
        again = false;
        const [m, o, b] = await Promise.all([fetchMarket(def.market_id), fetchOdds(def.market_id), fetchBets(def.market_id)]);
        if (alive) { if (m) setMarket(m); setOdds(o); setBets(b); }
      } while (alive && again);
      refreshing = false;
    };
    const source = new EventSource(streamUrl(def.market_id));
    source.onopen = () => { setConnected(true); void refresh(); };
    source.onerror = () => setConnected(false);
    for (const name of ["odds_updated", "bet_counted", "bet_status_changed", "market_status_changed", "payout_updated"]) source.addEventListener(name, refresh);
    return () => { alive = false; source.close(); };
  }, [def.market_id]);
  const sides = def.outcomes.map(o => ({ label: o.label, probability: odds?.pool_total_zat ? odds.outcomes.find(x => x.outcome_index === o.outcome_index)?.probability ?? null : null }));
  return <article className="bettor-page">
    <p className="section-index">S-02 / THE PUBLIC LEDGER</p><div className="ledger-title"><h1 className="t-2">{def.question}</h1><StatusBadge status={market.status} /></div>
    <p className="num t-5 muted">{def.network.toUpperCase()} / CLOSE {def.close_height.toLocaleString("en-US")} / RESOLVE {def.resolve_height.toLocaleString("en-US")}</p>
    <div className="market-detail-grid"><div>
      <section className="ledger-panel" aria-labelledby="odds-title"><div className="ledger-title"><h2 id="odds-title" className="t-3">Public odds</h2><p className="num t-5 muted" role="status">{connected ? "LIVE" : "RECONNECTING"}</p></div>{sides.length === 2 ? <OddsBar yes={sides[0]} no={sides[1]} /> : sides.map(s => <p className="t-4" key={s.label}>{s.label} <MonoNumber>{formatPercent(s.probability)}</MonoNumber></p>)}<p className="t-3 mt-6"><MonoNumber>{formatZec(odds?.pool_total_zat)}</MonoNumber> ZEC in the pool.</p><dl className="outcome-pools t-5">{def.outcomes.map(o => <div key={o.outcome_index}><dt>{o.label}</dt><dd className="num">{formatZec(odds?.outcomes.find(x => x.outcome_index === o.outcome_index)?.pool_zat)} ZEC</dd></div>)}</dl></section>
      <section className="ledger-panel"><h2 className="t-3">Anonymous feed</h2><p className="t-5 muted mt-2">Amounts. Outcomes. Blocks. No sender identities.</p>{bets?.slice(-20).reverse().map(b => <div className="anonymous-row t-5" key={b.note_ref}><MonoNumber>{formatZec(b.amount_zat)} ZEC</MonoNumber><span>{def.outcomes.find(o => o.outcome_index === b.outcome_index)?.label ?? "Unknown outcome"}</span><MonoNumber>Block {b.block_height.toLocaleString("en-US")}</MonoNumber></div>)}<p className="t-4 muted mt-4">{bets === null ? "Feed unavailable." : !bets.length ? "No bets observed." : ""}</p></section>
      <section className="ledger-panel"><h2 className="t-3">Locked rules</h2><p className="t-4 muted mt-3">{def.resolution_type} · <MonoNumber>{def.fee_bps / 100}%</MonoNumber> operator fee. Locked at announcement block <MonoNumber>{market.announce_height ?? "—"}</MonoNumber>.</p><pre className="ledger-code num t-5">{JSON.stringify(def.resolution_params, null, 2)}</pre><p className="t-5 muted">rules_hash</p><p className="ledger-hash num t-5">{market.rules_hash}</p><details className="mt-5"><summary className="t-4">Full market definition</summary><pre className="ledger-code num t-5">{JSON.stringify(def, null, 2)}</pre></details></section>
      <section className="ledger-panel"><h2 className="t-3">Verify yourself</h2><p className="t-4 muted mt-3">The public viewing key reveals escrow notes. It does not reveal their senders.</p><p className="ledger-hash num t-5 mt-4">{def.uivk}</p><Link className="text-link t-4" href={`/verify/${encodeURIComponent(def.market_id)}`}>Verify yourself</Link></section>
    </div><aside><BetBuilder market={market} /></aside></div>
  </article>;
}
