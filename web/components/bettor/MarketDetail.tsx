"use client";
import Link from "next/link";
import { useEffect, useState } from "react";
import { BetBuilder } from "./BetBuilder";
import { Settlement } from "./Settlement";
import { Scene3D } from "@/components/ui/Scene3D";
import { OddsBar } from "@/components/ui/OddsBar";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { MonoNumber } from "@/components/ui/MonoNumber";
import { BlockRef, TxRef } from "@/components/ui/ChainRef";
import { hasExplorer } from "@/lib/explorer";
import { fetchTip, streamUrl, formatPercent, formatZec } from "@/lib/api";
import { exactZec, fetchMarket, fetchOdds, fetchBets, fetchPayout, fetchResolution, type Market, type Odds, type PublicBet, type Payout, type Resolution } from "@/lib/bettor";

const BLOCK_SECONDS = 75;
const COMPARATORS: Record<string, string> = { gte: "at least", gt: "more than", lte: "at most", lt: "less than", eq: "exactly" };
function approxDuration(blocks: number) {
  const minutes = Math.round((blocks * BLOCK_SECONDS) / 60);
  if (minutes < 90) return `about ${Math.max(minutes, 1)} min`;
  const hours = minutes / 60;
  return hours < 48 ? `about ${Math.round(hours)} h` : `about ${Math.round(hours / 24)} days`;
}
function untilBlock(height: number, tip: number | null) {
  if (tip === null) return `block ${height.toLocaleString("en-US")}`;
  const left = height - tip;
  return left <= 0 ? `block ${height.toLocaleString("en-US")} (passed)` : `block ${height.toLocaleString("en-US")} (${left.toLocaleString("en-US")} blocks, ${approxDuration(left)})`;
}
/** The rules in a sentence. The JSON below it stays the source of truth. */
function plainRules(def: Market["market_definition"]) {
  const p = def.resolution_params as Record<string, unknown>;
  if (def.resolution_type === "COMMITTEE") return typeof p.criteria === "string" ? `A committee decides. ${p.criteria}` : "A committee decides the outcome.";
  if (typeof p.pool === "string" && typeof p.threshold_zat === "number") {
    const cmp = COMPARATORS[String(p.comparator)] ?? String(p.comparator);
    return `Decided by chain data, with no human vote: the outcome follows whether the ${p.pool} pool holds ${cmp} ${exactZec(p.threshold_zat)} ZEC when the market resolves.`;
  }
  return "Decided automatically from chain data, with no human vote.";
}

export function MarketDetail({ initialMarket, initialOdds, initialBets, initialResolution = null, initialPayout = null }: { initialMarket: Market; initialOdds: Odds | null; initialBets: PublicBet[] | null; initialResolution?: Resolution | null; initialPayout?: Payout | null }) {
  const [market, setMarket] = useState(initialMarket);
  const [odds, setOdds] = useState(initialOdds);
  const [bets, setBets] = useState(initialBets);
  const [connected, setConnected] = useState(false);
  const [resolution, setResolution] = useState(initialResolution);
  const [payout, setPayout] = useState(initialPayout);
  const [tip, setTip] = useState<number | null>(null);
  const def = market.market_definition;
  // The countdowns are in blocks, so they follow the chain tip.
  useEffect(() => {
    let alive = true;
    const poll = async () => { const h = await fetchTip(); if (alive) setTip(h); };
    void poll();
    const id = setInterval(poll, 10_000);
    return () => { alive = false; clearInterval(id); };
  }, []);
  useEffect(() => {
    let alive = true, refreshing = false, again = false;
    const refresh = async () => {
      if (refreshing) { again = true; return; }
      refreshing = true;
      do {
        again = false;
        const [m, o, b, r, p] = await Promise.all([fetchMarket(def.market_id), fetchOdds(def.market_id), fetchBets(def.market_id), fetchResolution(def.market_id), fetchPayout(def.market_id)]);
        if (alive) { if (m) setMarket(m); setOdds(o); setBets(b); setResolution(r); setPayout(p); }
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
    <div className="detail-top"><div><p className="section-index">S-02 / THE PUBLIC LEDGER</p><div className="ledger-title"><h1 className="t-2">{def.question}</h1><StatusBadge status={market.status} /></div></div><Scene3D kind="towers" yes={sides[0]?.probability ?? null} no={sides[1]?.probability ?? null} /></div>
    <p className="num t-5 muted">{def.network.toUpperCase()} / CLOSE {def.close_height.toLocaleString("en-US")} / RESOLVE {def.resolve_height.toLocaleString("en-US")}</p>
    <p className="t-4 mt-3">Bets close at {untilBlock(def.close_height, tip)}. The result is decided at {untilBlock(def.resolve_height, tip)}. {plainRules(def)}</p>
    <div className="market-detail-grid"><div>
      <section className="ledger-panel" aria-labelledby="odds-title"><div className="ledger-title"><h2 id="odds-title" className="t-3">Public odds</h2><p className="num t-5 muted" role="status">{connected ? "LIVE" : "RECONNECTING"}</p></div>{sides.length === 2 ? <OddsBar yes={sides[0]} no={sides[1]} /> : sides.map(s => <p className="t-4" key={s.label}>{s.label} <MonoNumber>{formatPercent(s.probability)}</MonoNumber></p>)}<p className="t-3 mt-6"><MonoNumber>{formatZec(odds?.pool_total_zat)}</MonoNumber> ZEC in the pool.</p><dl className="outcome-pools t-5">{def.outcomes.map(o => <div key={o.outcome_index}><dt>{o.label}</dt><dd className="num">{formatZec(odds?.outcomes.find(x => x.outcome_index === o.outcome_index)?.pool_zat)} ZEC</dd></div>)}</dl></section>
      <section className="ledger-panel"><h2 className="t-3">Anonymous feed</h2><p className="t-5 muted mt-2">Amounts. Outcomes. Blocks. No sender identities.</p>{bets?.slice(-20).reverse().map(b => <div className="anonymous-row t-5" key={b.note_ref}><MonoNumber>{formatZec(b.amount_zat)} ZEC</MonoNumber><span>{def.outcomes.find(o => o.outcome_index === b.outcome_index)?.label ?? "Unknown outcome"}</span><span>Block <BlockRef height={b.block_height} /></span></div>)}<p className="t-4 muted mt-4">{bets === null ? "Feed unavailable." : !bets.length ? "No bets observed." : ""}</p></section>
      <Settlement definition={def} resolution={resolution} payout={payout} tip={tip} />
      <section className="ledger-panel" aria-labelledby="chain-title"><h2 id="chain-title" className="t-3">On-chain proof</h2>
        <p className="t-4 muted mt-3">Every step of this market is a real {def.network} Zcash transaction. {hasExplorer ? "Open them in a public explorer to see that they exist and when they were mined." : "This is a local network, so there is no public explorer; the same ids work with your own node."} Shielded transactions hide amounts and people, so an explorer shows only that they happened.</p>
        <dl className="chain-proof t-5 mt-4">
          <div><dt>Market announced (rules locked on-chain)</dt><dd>{market.announce_txid ? <TxRef txid={market.announce_txid} /> : <span className="muted">Not announced yet.</span>}{market.announce_height != null && <span className="num"> · block <BlockRef height={market.announce_height} /></span>}</dd></div>
          <div><dt>Bets</dt><dd className="t-5">Each bet is a shielded transaction in the block shown in the feed above. Bet transaction ids are not published, so bets cannot be linked to anyone.</dd></div>
          <div><dt>Payouts</dt><dd>{payout?.txs?.length ? payout.txs.map(t => <div key={t.txid}><TxRef txid={t.txid} /></div>) : <span className="muted">Not paid out yet.</span>}</dd></div>
        </dl></section>
      <section className="ledger-panel" aria-labelledby="checker-title"><h2 id="checker-title" className="t-3">For checkers</h2>
        <p className="t-4 muted mt-3">You do not need this to bet. It is the raw proof that the rules cannot change and that anyone can recompute the result.</p>
        <details className="mt-5"><summary className="t-4">Locked rules</summary>
          <p className="t-4 muted mt-3">{def.resolution_type} · <MonoNumber>{def.fee_bps / 100}%</MonoNumber> operator fee. Locked at announcement block <MonoNumber>{market.announce_height ?? "—"}</MonoNumber>. The rules_hash is a fingerprint of these rules: change one character and it changes.</p>
          <pre className="ledger-code num t-5">{JSON.stringify(def.resolution_params, null, 2)}</pre><p className="t-5 muted">rules_hash</p><p className="ledger-hash num t-5">{market.rules_hash}</p></details>
        <details className="mt-5"><summary className="t-4">Full market definition</summary><pre className="ledger-code num t-5">{JSON.stringify(def, null, 2)}</pre></details>
        <details className="mt-5"><summary className="t-4">Viewing key</summary><p className="t-4 muted mt-3">The public viewing key reveals escrow notes. It does not reveal their senders.</p><p className="ledger-hash num t-5 mt-4">{def.uivk}</p></details>
        <p className="mt-5"><Link className="text-link t-4" href={`/verify/${encodeURIComponent(def.market_id)}`}>Verify yourself</Link></p></section>
    </div><aside><BetBuilder market={market} /></aside></div>
  </article>;
}
