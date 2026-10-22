"use client";
// docs/DESIGN.md. Private receipts stay on this device; only public market paths are fetched.
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/Button";
import { MonoNumber } from "@/components/ui/MonoNumber";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { Dialog } from "@/components/ui/primitives";
import { fetchTip } from "@/lib/api";
import { fetchBets, fetchMarket, fetchPayout, exactZec } from "@/lib/bettor";
import { BET_EXPLANATIONS } from "@/lib/bet-status";
import { checkPlan, downloadJson, importReceipts, listReceipts, matchReceipt, saveReceipt, type BetReceipt } from "@/lib/receipts";
import { loadGlassWasm } from "@/lib/wasm";

export function MyBets() {
  const [receipts, setReceipts] = useState<BetReceipt[]>([]);
  const [message, setMessage] = useState("Loading local receipts…");
  const [checks, setChecks] = useState<Record<string, string>>({});
  const [confirmations, setConfirmations] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [marketLabels, setMarketLabels] = useState<Record<string, { question: string; outcomes: { outcome_index: number; label: string }[] }>>({});
  const [blocks, setBlocks] = useState<Record<string, number>>({});
  const refresh = useCallback(async () => {
    setBusy(true);
    try {
      const local = await listReceipts(); setReceipts(local);
      const ids = [...new Set(local.map(r => r.market_id))];
      const publicData = new Map(await Promise.all(ids.map(async id => [id, await fetchBets(id)] as const)));
      const markets = new Map(await Promise.all(ids.map(async id => [id, await fetchMarket(id)] as const)));
      const updated = local.map(r => { const bets = publicData.get(r.market_id); return bets ? matchReceipt(r, bets) : { ...r, last_status: "UNAVAILABLE" }; });
      await Promise.all(updated.map(saveReceipt)); setReceipts(updated);
      const conf: Record<string, string> = {}, observed: Record<string, number> = {};
      const tip = updated.some(r => r.last_status === "PENDING") ? await fetchTip() : null;
      for (const r of updated) {
        const market = markets.get(r.market_id), bet = publicData.get(r.market_id)?.find(b => b.note_ref === r.note_ref);
        if (bet) observed[r.bet_fingerprint] = bet.block_height;
        if (r.last_status === "PENDING" && market && bet) conf[r.bet_fingerprint] = `Confirming (${tip === null ? "—" : Math.min(market.market_definition.min_confirmations, Math.max(0, tip - bet.block_height + 1))}/${market.market_definition.min_confirmations})`;
      }
      setConfirmations(conf); setBlocks(observed);
      setMarketLabels(Object.fromEntries([...markets].filter(([, m]) => m !== null).map(([id, m]) => [id, { question: m!.market_definition.question, outcomes: m!.market_definition.outcomes }]))); setMessage(updated.length ? "Public statuses refreshed. Receipts stay on this device." : "No receipts saved on this device.");
    } catch { setMessage("Local storage unavailable. Check browser storage permissions. Existing receipts were not deleted."); }
    finally { setBusy(false); }
  }, []);
  useEffect(() => { const timer = setTimeout(() => { void refresh(); }, 0); return () => clearTimeout(timer); }, [refresh]);
  async function importFile(file: File | undefined) {
    if (!file) return;
    try { if (file.size > 5_000_000) throw new Error("Receipt file exceeds 5 MB."); await importReceipts(await file.text()); await refresh(); }
    catch (e) { setMessage(e instanceof Error ? e.message : "Import failed."); }
  }
  async function plan(receipt: BetReceipt) {
    try {
      const payout = await fetchPayout(receipt.market_id);
      const result = !payout ? "No public plan available." : checkPlan(receipt, payout.plan.items, (await loadGlassWasm()).receiver_commitment(receipt));
      setChecks(c => ({ ...c, [receipt.bet_fingerprint]: result }));
    } catch { setMessage("Plan check failed. Retry when the public API and local WASM are available."); }
  }
  async function proof(receipt: BetReceipt) {
    try { if (!receipt.note_ref) throw new Error("Match the on-chain note first."); const wasm = await loadGlassWasm(); downloadJson(wasm.envelope_proof(receipt, receipt.note_ref), `envelope-proof-${receipt.bet_fingerprint}.json`); }
    catch { setMessage("Proof export failed. Check the local WASM and receipt."); }
  }
  return <section className="bettor-page"><p className="section-index">S-04 / THIS DEVICE</p><h1 className="t-2 my-6">My Bets.</h1><p className="t-4 muted">Clearing browser data deletes these receipts. Export them.</p><p className="t-5 muted mt-2">Receipt files contain payout addresses and envelope secrets. Keep backups private.</p>
    <div className="ledger-actions my-6"><Button variant="ghost" disabled={busy} onClick={refresh}>Refresh statuses</Button><Button variant="ghost" disabled={!receipts.length} onClick={() => downloadJson(receipts, "frosted-receipts.json")}>Export receipts</Button><label className="t-4" htmlFor="receipt-import">Import receipts<input id="receipt-import" className="ledger-input" type="file" accept="application/json,.json" onChange={e => { void importFile(e.target.files?.[0]); e.target.value = ""; }} /></label></div>
    <p role="status" className="t-4">{message}</p>
    {receipts.map(r => <article className="receipt-row" key={r.bet_fingerprint}><div className="ledger-title"><Link href={`/markets/${r.market_id}`} className="text-link num t-5">{marketLabels[r.market_id]?.question ?? `Market ${r.market_id}`}</Link><StatusBadge status={r.last_status} /></div><p className="num t-5 mt-4">{exactZec(r.amount_zat)} ZEC / {marketLabels[r.market_id]?.outcomes.find(o => o.outcome_index === r.outcome_index)?.label ?? `OUTCOME ${r.outcome_index}`} / {r.bet_fingerprint}</p>{blocks[r.bet_fingerprint] !== undefined && <p className="num t-5 muted">Block {blocks[r.bet_fingerprint].toLocaleString("en-US")}</p>}{r.note_ref && <p className="num t-5 muted">Note {r.note_ref}</p>}<p className="t-4 mt-3">{BET_EXPLANATIONS[r.last_status] ?? "Public status unavailable."}</p>{confirmations[r.bet_fingerprint] && <p className="num t-5">{confirmations[r.bet_fingerprint]}</p>}{r.amount_mismatch && <div className="mt-4"><StatusBadge status="AMOUNT_MISMATCH" /><p className="t-4 mt-2">{BET_EXPLANATIONS.AMOUNT_MISMATCH}</p></div>}<div className="ledger-actions mt-5"><Button variant="ghost" disabled={!r.note_ref} onClick={() => plan(r)}>Check my plan item</Button><Dialog title="Reveal the payout receiver?" description="This proof reveals your payout receiver, bet salt and envelope secret. Share it only to dispute a false envelope claim. It does not reveal the sender." trigger={<Button variant="ghost" disabled={!r.note_ref}>Export Envelope Proof</Button>}><Button onClick={() => proof(r)}>Export proof</Button></Dialog></div>{checks[r.bet_fingerprint] && <p role="status" className="num t-5 mt-4">{checks[r.bet_fingerprint]}</p>}<p className="t-5 muted mt-3">{r.tagged ? "Persona tagged" : "Untagged"} · Saved <MonoNumber>{new Date(r.created_at).toISOString()}</MonoNumber></p></article>)}
  </section>;
}
