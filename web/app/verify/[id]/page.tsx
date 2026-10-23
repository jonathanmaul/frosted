import Link from "next/link";
import { ServerCheck } from "@/components/verify/ServerCheck";
import { fetchMarket } from "@/lib/bettor";
import { verifyCommands } from "@/lib/verify";

export default async function Page({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const market = await fetchMarket(id);
  return <section className="bettor-page"><p className="section-index">S-10 / VERIFY</p><h1 className="t-2 my-6">Verify this market.</h1>
    <p className="t-4 muted">{market ? market.market_definition.question : <>Market <span className="num">{id}</span></>}</p>
    <div className="ledger-panel"><h2 className="t-3">Run it yourself</h2>
      <p className="t-4 muted mt-2">Needs only your own Zcash node and the public API. The verifier scans the chain with the market&apos;s published viewing key and recomputes every claim: the rules hash, the addresses, the pools, the Chain-Verified outcome, and each item of the payout plan. It holds no secret and cannot see who bet.</p>
      {verifyCommands(id).map((c) => <div key={c.command}><p className="t-5 muted mt-4">{c.label}</p><p className="ledger-code num t-5">{c.command}</p></div>)}
      <p className="t-5 muted">Add <span className="num">--json</span> for machine-readable output. Exit code 0 means every check passed; 1 means at least one failed.</p></div>
    <div className="ledger-panel"><h2 className="t-3">Disprove an UNCLAIMABLE label</h2>
      <p className="t-4 muted mt-2">If My Bets shows your winning bet as UNCLAIMABLE, export an Envelope Proof there. Anyone can then check it against the memo on their own node. Publishing it reveals your payout receiver, never your sender.</p>
      <p className="ledger-code num t-5">frosted-verify envelope-proof proof.json --node {"http://127.0.0.1:8232"}</p></div>
    <ServerCheck marketId={id} />
    <p className="t-4 mt-6"><Link className="text-link" href={`/markets/${id}`}>Back to the market</Link></p>
  </section>;
}
