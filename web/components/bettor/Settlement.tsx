// Public settlement ledger. Flat surfaces and type scale follow docs/DESIGN.md.
import { MonoNumber } from "@/components/ui/MonoNumber";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { CommitteePanel } from "./CommitteePanel";
import { exactZec, type MarketDefinition, type Payout, type Resolution } from "@/lib/bettor";

export function SigningProgress({ signing }: { signing: Payout["signing"] }) {
  return <div className="mt-4">
    <p className="num t-5">Signing: {signing.sessions_done} / {signing.sessions_total} sessions · {signing.threshold} of 3 members</p>
    <progress className="settlement-progress mt-2 w-full" aria-label="Completed signing sessions" max={Math.max(1, signing.sessions_total)} value={signing.sessions_done} />
  </div>;
}

export function Settlement({ definition, resolution, payout, tip = null }: { definition: MarketDefinition; resolution: Resolution | null; payout: Payout | null; tip?: number | null }) {
  const evidence = resolution?.evidence;
  const winner = resolution?.winning_outcome_index;
  return <section className="ledger-panel" aria-labelledby="settlement-title">
    <p className="section-index">S-05 / SETTLEMENT</p>
    <h2 id="settlement-title" className="t-3">Resolution & payout</h2>
    <p className="t-5 muted mt-3">{definition.resolution_type}</p>
    {resolution ? <div className="mt-4">
      <StatusBadge status={resolution.status} />
      <p className="t-4 mt-3">Outcome: {winner === null ? resolution.status === "VOID" ? "VOID" : "—" : definition.outcomes.find(o => o.outcome_index === winner)?.label ?? "—"}</p>
      {evidence && <div className="mt-4">
        <p className="num t-5">{evidence.source} / Block {evidence.height}</p>
        <p className="ledger-hash num t-5 mt-2">{evidence.block_hash}</p>
        {evidence.chain_value_zat !== undefined && <p className="t-4 mt-3">{evidence.pool}: <MonoNumber>{exactZec(evidence.chain_value_zat)} ZEC</MonoNumber></p>}
        {evidence.reported_upgrades && <details className="mt-3"><summary className="t-4">Upgrades reported by the node</summary><ul className="num t-5 mt-3">{evidence.reported_upgrades.map(u => <li key={u.name}>{u.name} / Activation block {u.activation_height}</li>)}</ul></details>}
      </div>}
    </div> : <p className="t-4 muted mt-4">No public resolution published.</p>}
    <CommitteePanel definition={definition} resolution={resolution} tip={tip} />
    {payout ? <div className="mt-6">
      <div className="ledger-title"><h3 className="t-3">Public payout plan</h3><StatusBadge status={payout.status} /></div>
      <p className="ledger-hash num t-5 mt-3">plan_hash: {payout.plan_hash}</p>
      <p className="num t-5 mt-3">{payout.plan.plan_kind} / Cutoff block {payout.plan.plan_cutoff_height}</p>
      <p className="num t-5 mt-3">{payout.summary.winners} winners / {payout.summary.refunds} refunds / {payout.summary.unclaimable} unclaimable / {payout.summary.swept} swept</p>
      <dl className="outcome-pools t-5 mt-4">
        <div><dt>Distributed</dt><dd className="num">{exactZec(payout.summary.distributed_zat)} ZEC</dd></div>
        <div><dt>Operator fee</dt><dd className="num">{exactZec(payout.summary.operator_fee_zat)} ZEC</dd></div>
        <div><dt>Network fee</dt><dd className="num">{exactZec(payout.summary.tx_fee_zat)} ZEC</dd></div>
      </dl>
      <SigningProgress signing={payout.signing} />
      <details className="mt-5"><summary className="t-4">Public plan items</summary><div className="overflow-x-auto mt-3"><table className="num t-5 w-full text-left"><caption className="t-5 muted mb-3">Note references and receiver commitments. No payout addresses.</caption><thead><tr><th scope="col">Note</th><th scope="col">Kind</th><th scope="col">Amount (ZEC)</th><th scope="col">Receiver commitment</th></tr></thead><tbody>{payout.plan.items.map(item => <tr key={item.note_ref}><td className="py-2 pr-4">{item.note_ref}</td><td className="pr-4">{item.kind}</td><td className="pr-4">{exactZec(item.amount_zat)}</td><td>{item.receiver_commitment ?? "Treasury"}</td></tr>)}</tbody></table></div></details>
      <h3 className="t-3 mt-6">Payout transactions</h3>
      <p className="t-5 muted mt-2">These are escrow payout transaction IDs.</p>
      {payout.txs.length ? payout.txs.map(tx => <div className="mt-4" key={tx.txid}><StatusBadge status={tx.status} /><p className="ledger-hash num t-5 mt-2">{tx.txid}</p>{tx.confirmed_height !== null && <p className="num t-5 mt-2">Confirmed at block {tx.confirmed_height}</p>}</div>) : <p className="t-4 muted mt-3">No payout transaction published.</p>}
    </div> : <p className="t-4 muted mt-5">No public payout plan published.</p>}
    <p className="ledger-code num t-5 mt-6">frosted-verify all {definition.market_id}</p>
    <a className="text-link t-4" href="/operator">Operator status</a>
  </section>;
}
