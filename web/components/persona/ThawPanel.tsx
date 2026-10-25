"use client";

// S-07. `K_root` leaves this device only when the user presses Thaw, and never before.
import Link from "next/link";
import { PageHead } from "@/components/ui/Scene3D";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/Button";
import { MonoNumber } from "@/components/ui/MonoNumber";
import { postJson } from "@/lib/api";
import { decryptRoot, fetchMarketLabels, fetchPersona, getPersona, rootToBase64Url, validateAlias, type MarketLabel, type PersonaInfo, type ThawReport } from "@/lib/persona";
import type { PersonaRecord } from "@/lib/receipts";
import { ThawReportView } from "./ThawReportView";

const ERROR_COPY: Record<string, string> = {
  THAW_COMMITMENT_MISMATCH: "The stored key does not match the registered Persona. Nothing was published.",
  PERSONA_ALREADY_THAWED: "This Persona is already thawed. Its record is public.",
  PERSONA_NOT_FOUND: "The registry has not recorded this Persona yet. Wait for registration, then try again.",
};

export function ThawPanel() {
  const [record, setRecord] = useState<PersonaRecord | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [info, setInfo] = useState<PersonaInfo | null>(null);
  const [alias, setAlias] = useState("");
  const [confirmAlias, setConfirmAlias] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ThawReport | null>(null);
  const [markets, setMarkets] = useState<Record<string, MarketLabel>>({});

  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const persona = await getPersona();
        if (live) setRecord(persona);
        if (persona) { const current = await fetchPersona(persona.persona_id); if (live) setInfo(current); }
      } catch { if (live) setError("Local storage unavailable. Check browser storage permissions."); }
      finally { if (live) setLoaded(true); }
    })();
    return () => { live = false; };
  }, []);

  const aliasError = validateAlias(alias, confirmAlias);
  const ready = !!record && !!passphrase && !aliasError && info?.status === "REGISTERED" && !busy;
  async function thaw() {
    if (!record) return;
    setBusy(true); setError("");
    try {
      const root = await decryptRoot(record, passphrase);
      const result = await postJson<ThawReport>(`/personas/${record.persona_id}/thaw`, { k_root_b64: rootToBase64Url(root), alias });
      if (!result.ok) { setError(ERROR_COPY[result.code] ?? result.message); return; }
      setPassphrase("");
      setReport(result.data);
      setMarkets(await fetchMarketLabels(result.data.bets.map((b) => b.market_id)));
    } catch (e) { setError(e instanceof Error ? e.message : "Thaw failed."); }
    finally { setBusy(false); }
  }

  if (!loaded) return <section className="bettor-page"><p className="section-index">S-07 / THAW</p><p role="status" className="t-4 muted">Loading local Persona…</p></section>;
  if (report) return <section className="bettor-page"><PageHead index="S-07 / THAW REPORT" title={`Thawed: ${report.alias}.`} kind="crystal" lede={<>Registered at block <MonoNumber>{report.registered_height.toLocaleString("en-US")}</MonoNumber>. Your key is now public, and so is this record.</>} />
    <div className="ledger-panel"><ThawReportView report={report} markets={markets} /></div>
    <p className="t-4 mt-6"><Link className="text-link" href={`/p/${report.persona_id}`}>Open the public profile</Link></p></section>;
  return <section className="bettor-page"><PageHead index="S-07 / THAW" title="Thaw." kind="crystal" />
    {!record && <p className="t-4">No Persona on this device. <Link className="text-link" href="/persona">Create one first.</Link></p>}
    {record && info?.status === "THAWED" && <p className="t-4">This Persona is already thawed. <Link className="text-link" href={`/p/${record.persona_id}`}>Open the public profile.</Link></p>}
    {record && info?.status !== "THAWED" && <div className="ledger-panel">
      <p className="t-3">Thawing reveals ALL bets tagged by this Persona, including losses. This cannot be undone.</p>
      <p className="t-4 muted mt-2">Your key and alias become public. Bets from before registration, and copies of your tags, are listed but excluded from your record.</p>
      {info === null && <p role="status" className="t-4 mt-4">Registration not seen yet. Thaw opens after the registry records this Persona.</p>}
      <fieldset disabled={busy} className="builder-inputs mt-6"><legend className="t-3">Confirm</legend>
        <label htmlFor="thaw-alias">Public alias<input id="thaw-alias" className="ledger-input" autoComplete="off" spellCheck={false} value={alias} onChange={(e) => setAlias(e.target.value)} /></label>
        <label htmlFor="thaw-alias-confirm">Type the alias again to confirm<input id="thaw-alias-confirm" className="ledger-input" autoComplete="off" spellCheck={false} value={confirmAlias} onChange={(e) => setConfirmAlias(e.target.value)} /></label>
        <label htmlFor="thaw-passphrase">Passphrase<input id="thaw-passphrase" className="ledger-input" type="password" autoComplete="current-password" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} /></label>
        {alias && aliasError && <p className="t-5 muted">{aliasError}</p>}
        <div className="ledger-actions"><Button disabled={!ready} onClick={thaw}>{busy ? "Thawing…" : "Thaw permanently"}</Button></div></fieldset></div>}
    {error && <p role="alert" className="t-4 mt-4">{error}</p>}
  </section>;
}
