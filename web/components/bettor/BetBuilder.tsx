"use client";

// Four steps, local secrets only. Visual rules: docs/DESIGN.md.
import Image from "next/image";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/Button";
import { GlassCard } from "@/components/ui/GlassCard";
import { MonoNumber } from "@/components/ui/MonoNumber";
import type { Market } from "@/lib/bettor";
import { exactZec, parseAmount } from "@/lib/bettor";
import { decryptRoot, fetchPersona, getPersona } from "@/lib/persona";
import { saveReceipt, type PersonaRecord } from "@/lib/receipts";
import { loadGlassWasm, type BuiltBet } from "@/lib/wasm";

export function BetBuilder({ market }: { market: Market }) {
  const def = market.market_definition;
  const [step, setStep] = useState(0);
  const [outcome, setOutcome] = useState(def.outcomes[0]?.outcome_index ?? 0);
  const [amount, setAmount] = useState(exactZec(def.min_bet_zat));
  const [payout, setPayout] = useState("");
  const [persona, setPersona] = useState(false);
  const [root, setRoot] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [local, setLocal] = useState<PersonaRecord | null>(null);
  const [registered, setRegistered] = useState<boolean | null>(null);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [built, setBuilt] = useState<BuiltBet | null>(null);
  const [qr, setQr] = useState("");
  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const record = await getPersona(); if (!live) return;
        setLocal(record);
        if (record) { const info = await fetchPersona(record.persona_id); if (live) setRegistered(info !== null); }
      } catch { /* No local storage: the toggle simply stays unavailable. */ }
    })();
    return () => { live = false; };
  }, []);
  async function advance() {
    setBusy(true); setError("");
    try {
      if (step === 1 && parseAmount(amount) < def.min_bet_zat) throw new Error(`Minimum ${exactZec(def.min_bet_zat)} ZEC.`);
      if (step === 2) {
        const wasm = await loadGlassWasm();
        const parsed = wasm.parse_payout_ua(payout.trim(), def.network);
        if (!parsed.ok) throw new Error(parsed.error ?? "PAYOUT_ADDRESS_NO_ORCHARD");
        // Decrypt now so a wrong passphrase fails before sealing. The root stays in memory only.
        if (persona && !root) { if (!local) throw new Error("No Persona on this device."); setRoot(await decryptRoot(local, passphrase)); setPassphrase(""); }
      }
      setStep(step + 1);
    } catch (e) { setError(String(e instanceof Error ? e.message : e)); }
    finally { setBusy(false); }
  }
  async function seal() {
    setBusy(true); setError("");
    try {
      if (market.status !== "OPEN") throw new Error("Market is not open.");
      const amount_zat = parseAmount(amount);
      const wasm = await loadGlassWasm();
      const result = wasm.build_bet({ market_definition: def, outcome_index: outcome, amount_zat, payout_ua: payout.trim(), ...(persona ? { k_root: root } : {}) });
      // Never reveal a payable URI until its recovery receipt has committed to IndexedDB.
      await saveReceipt({ bet_fingerprint: result.bet_fingerprint, market_id: def.market_id, outcome_index: outcome, amount_zat,
        payout_ua: payout.trim(), bet_salt: result.bet_salt, env_esk: result.env_esk, tagged: persona,
        note_ref: null, created_at: Date.now(), last_status: "WAITING", amount_mismatch: false });
      const QRCode = await import("qrcode");
      const image = await QRCode.toDataURL(result.zip321_uri, { width: 512, margin: 4, errorCorrectionLevel: "M" });
      setRoot(""); setBuilt(result); setQr(image);
    } catch (e) { setError(String(e instanceof Error ? e.message : e)); }
    finally { setBusy(false); }
  }
  async function copy() {
    try { await navigator.clipboard.writeText(built!.zip321_uri); setNotice("URI copied."); }
    catch { setError("Clipboard unavailable. Select and copy the URI below."); }
  }
  if (market.status !== "OPEN") return <section className="ledger-panel" aria-label="Bet Builder"><h2 className="t-3">Bet Builder</h2><p className="t-4 muted">{market.status === "ANNOUNCING" ? "Waiting for market announcement." : <>Closed at block <MonoNumber>{def.close_height}</MonoNumber>.</>}</p></section>;
  return <GlassCard className="builder" id="bet-builder">
    <p className="section-index">S-03 / LOCAL BET BUILDER</p><h2 className="t-2">Seal your position.</h2>
    <ol className="builder-steps num t-5" aria-label="Bet steps">{["Outcome", "Amount", "Payout", "Seal & scan"].map((name, i) => <li key={name} aria-current={step === i ? "step" : undefined}>{i + 1} / {name}</li>)}</ol>
    <p className="t-4 muted">Send from a shielded address. Transparent inputs are rejected and refunded. The coordinator learns your payout address when opening the envelope.</p>
    {!built && <fieldset disabled={busy} className="builder-inputs">
      <legend className="t-3">{["01 / Choose an outcome", "02 / Set the amount", "03 / Payout address", "04 / Review & seal"][step]}</legend>
      {step === 0 && <div className="outcome-options">{def.outcomes.map(o => <label key={o.outcome_index}><input type="radio" name="outcome" checked={outcome === o.outcome_index} onChange={() => setOutcome(o.outcome_index)} /> {o.label}</label>)}</div>}
      {step === 1 && <label htmlFor="bet-amount">Amount in ZEC<input id="bet-amount" className="ledger-input num" inputMode="decimal" value={amount} onChange={e => setAmount(e.target.value)} /><span className="num t-5 muted">Minimum {exactZec(def.min_bet_zat)} ZEC</span></label>}
      {step === 2 && <><label htmlFor="payout-ua">Payout Unified Address ({def.network})<span className="block t-5 muted">Use a fresh address.</span><textarea id="payout-ua" className="ledger-input num" rows={3} autoComplete="off" spellCheck={false} value={payout} onChange={e => setPayout(e.target.value)} /></label>{local ? <label><input type="checkbox" checked={persona} onChange={e => { setPersona(e.target.checked); if (!e.target.checked) { setRoot(""); setPassphrase(""); } }} /> Tag this bet with my Persona</label> : <p className="t-5 muted">No Persona on this device. <Link className="text-link" href="/persona">Create one</Link> to tag bets.</p>}{persona && local && <><label htmlFor="persona-passphrase">Persona passphrase<input id="persona-passphrase" className="ledger-input" type="password" autoComplete="current-password" value={passphrase} onChange={e => setPassphrase(e.target.value)} /><span className="t-5 muted">Decrypts your Persona key on this device. Kept in memory only. Never shared with the API.</span></label>{registered === false && <p className="t-5">This Persona is not registered yet. Bets placed before registration are excluded from your record.</p>}</>}</>}
      {step === 3 && <p className="t-4">{def.outcomes.find(o => o.outcome_index === outcome)?.label} · <MonoNumber>{amount} ZEC</MonoNumber> · {persona ? "Persona tagged" : "Untagged"}.<br />Save a receipt backup from My Bets after sealing. Clearing browser data deletes receipts.</p>}
      <div className="ledger-actions">{step > 0 && <Button variant="ghost" onClick={() => { setStep(step - 1); setError(""); }}>Back</Button>}<Button onClick={step === 3 ? seal : advance}>{busy ? "Working locally…" : step === 3 ? "Seal & save receipt" : "Continue"}</Button></div>
    </fieldset>}
    {built && <div className="sealed-bet"><p className="t-3">Receipt saved on this device.</p><p className="num t-5">Fingerprint {built.bet_fingerprint}</p>{/* Data URI generated locally; no image service receives the payment URI. */}<Image unoptimized src={qr} width={256} height={256} alt="ZIP-321 payment QR. The same URI is available below." /><Button onClick={copy}>Copy URI</Button><label htmlFor="payment-uri" className="t-5">Payment URI<textarea id="payment-uri" className="ledger-input num" readOnly rows={4} value={built.zip321_uri} /></label><a className="text-link t-4" href="/my-bets">Open My Bets</a><p className="t-4 muted">Keep the wallet amount unchanged. A changed amount breaks the Persona tag.</p></div>}
    {error && <p role="alert" className="t-4">{error}</p>}<p role="status" className="t-4">{notice}</p>
  </GlassCard>;
}
