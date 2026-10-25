import Link from "next/link";
import { PageHead } from "@/components/ui/Scene3D";
import { ServerCheck } from "@/components/verify/ServerCheck";
import { fetchMarket, fetchPayout } from "@/lib/bettor";
import { verifyCommands } from "@/lib/verify";
import { BlockRef, TxRef } from "@/components/ui/ChainRef";

export default async function Page({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const [market, payout] = await Promise.all([fetchMarket(id), fetchPayout(id)]);
  return <section className="bettor-page"><PageHead index="S-10 / VERIFY" title="Verify this market." kind="chain" />
    <p className="t-4 muted">{market ? market.market_definition.question : <>Market <span className="num">{id}</span></>}</p>
    <div className="ledger-panel"><h2 className="t-3">Run it yourself</h2>
      <p className="t-4 muted mt-2">Needs only your own Zcash node and the public API. The verifier scans the chain with the market&apos;s published viewing key and recomputes every claim: the rules hash, the addresses, the pools, the Chain-Verified outcome, and each item of the payout plan. It holds no secret and cannot see who bet.</p>
      {verifyCommands(id).map((c) => <div key={c.command}><p className="t-5 muted mt-4">{c.label}</p><p className="ledger-code num t-5">{c.command}</p></div>)}
      <p className="t-5 muted">Add <span className="num">--json</span> for machine-readable output. Exit code 0 means every check passed; 1 means at least one failed.</p></div>
    <div className="ledger-panel"><h2 className="t-3">Disprove an UNCLAIMABLE label</h2>
      <p className="t-4 muted mt-2">If My Bets shows your winning bet as UNCLAIMABLE, export an Envelope Proof there. Anyone can then check it against the memo on their own node. Publishing it reveals your payout receiver, never your sender.</p>
      <p className="ledger-code num t-5">frosted-verify envelope-proof proof.json --node {"http://127.0.0.1:8232"}</p></div>
    <div className="ledger-panel"><h2 className="t-3">On-chain evidence</h2>
      <p className="t-4 muted mt-2">Every bet and payout is a real shielded Zcash transaction. A public explorer shows only that a transaction exists, never its amount or parties. Bet transaction IDs are deliberately not published, so the anonymous feed cannot be matched to anyone&apos;s wallet history. You can find your own in your wallet.</p>
      {market?.announce_txid && <><h3 className="t-4 mt-4">Announcement transaction</h3><p className="mt-2"><TxRef txid={market.announce_txid} />{market.announce_height != null && <span className="num t-5 muted"> · block <BlockRef height={market.announce_height} /></span>}</p></>}
      {market && <><h3 className="t-4 mt-4">Escrow addresses (one per outcome)</h3>
        <ul className="verify-list">{(market.market_definition.outcomes ?? []).map(o => <li key={o.outcome_index}><p className="t-5">{o.label}</p><p className="ledger-hash num t-5">{o.address}</p></li>)}</ul>
        <h3 className="t-4 mt-4">Viewing key</h3><p className="ledger-hash num t-5">{market.market_definition.uivk}</p></>}
      <h3 className="t-4 mt-4">Payout transactions ({payout?.txs?.length ?? 0})</h3>
      {payout?.txs?.length ? <ul className="verify-list">{payout.txs.map(t => <li key={t.txid}><TxRef txid={t.txid} /> <span className="muted t-5">{t.status}{t.confirmed_height !== null ? `, block ${t.confirmed_height}` : ""}</span></li>)}</ul> : <p className="t-5 muted mt-2">No payout transaction published yet.</p>}
      <p className="t-5 muted mt-4">Check a payout on your own node:</p>
      <p className="ledger-code num t-5">zcash-cli getrawtransaction &lt;txid&gt; 1</p></div>
    <ServerCheck marketId={id} />
    <p className="t-4 mt-6"><Link className="text-link" href={`/markets/${id}`}>Back to the market</Link></p>
  </section>;
}
