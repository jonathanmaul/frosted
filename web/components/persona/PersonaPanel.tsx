"use client";

// S-06. Visual rules: docs/DESIGN.md. `K_root` is shown once for backup, then kept only encrypted.
import Image from "next/image";
import Link from "next/link";
import { PageHead } from "@/components/ui/Scene3D";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/Button";
import { MonoNumber } from "@/components/ui/MonoNumber";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { decryptRoot, encryptRoot, fetchPersona, fetchRegistry, getPersona, MIN_PASSPHRASE, savePersona, taggedBetCount, type PersonaInfo } from "@/lib/persona";
import { downloadJson, type PersonaRecord } from "@/lib/receipts";
import { loadGlassWasm } from "@/lib/wasm";

type Draft = { k_root: string; persona_id: string; frstp1_uri: string };
const POLL_MS = 10_000;

function Qr({ uri }: { uri: string }) {
  const [image, setImage] = useState("");
  useEffect(() => {
    let live = true;
    // Generated locally: no image service sees the registration URI.
    void import("qrcode").then((QR) => QR.toDataURL(uri, { width: 512, margin: 4, errorCorrectionLevel: "M" })).then((d) => { if (live) setImage(d); }).catch(() => {});
    return () => { live = false; };
  }, [uri]);
  return <div className="sealed-bet">{image && <Image unoptimized src={image} width={256} height={256} alt="Persona registration QR. The same URI is available below." />}
    <label htmlFor="registration-uri" className="t-5">Registration URI<textarea id="registration-uri" className="ledger-input num" readOnly rows={3} value={uri} /></label></div>;
}

export function PersonaPanel() {
  const [record, setRecord] = useState<PersonaRecord | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [info, setInfo] = useState<PersonaInfo | null>(null);
  const [tagged, setTagged] = useState(0);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [saved, setSaved] = useState(false);
  const [passphrase, setPassphrase] = useState("");
  const [confirm, setConfirm] = useState("");
  const [backup, setBackup] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let live = true;
    void (async () => {
      try { const [persona, count] = await Promise.all([getPersona(), taggedBetCount()]); if (live) { setRecord(persona); setTagged(count); } }
      catch { if (live) setError("Local storage unavailable. Check browser storage permissions."); }
      finally { if (live) setLoaded(true); }
    })();
    return () => { live = false; };
  }, []);
  const personaId = record?.persona_id;
  const poll = useCallback(async () => { if (personaId) setInfo(await fetchPersona(personaId)); }, [personaId]);
  useEffect(() => {
    if (!personaId) return;
    const first = setTimeout(() => { void poll(); }, 0), timer = setInterval(() => { void poll(); }, POLL_MS);
    return () => { clearTimeout(first); clearInterval(timer); };
  }, [personaId, poll]);

  async function create() {
    setBusy(true); setError("");
    try {
      const registry = await fetchRegistry();
      if (!registry) throw new Error("The Persona Registry is not published yet. Try again later.");
      const wasm = await loadGlassWasm();
      setDraft(wasm.new_persona(registry.address)); setSaved(false); setPassphrase(""); setConfirm("");
    } catch (e) { setError(e instanceof Error ? e.message : "Could not create a Persona."); }
    finally { setBusy(false); }
  }
  // Nothing is stored, and no registration QR exists, until the backup is confirmed.
  async function confirmBackup() {
    setBusy(true); setError("");
    try {
      if (!draft) return;
      if (!saved) throw new Error("Confirm that you saved the backup key.");
      if (passphrase !== confirm) throw new Error("The passphrases do not match.");
      const next: PersonaRecord = { persona_id: draft.persona_id, frstp1_uri: draft.frstp1_uri, created_at: Date.now(), enc: await encryptRoot(draft.k_root, passphrase, draft.persona_id) };
      await savePersona(next);
      setRecord(next); setDraft(null); setPassphrase(""); setConfirm("");
    } catch (e) { setError(e instanceof Error ? e.message : "Could not store the Persona."); }
    finally { setBusy(false); }
  }
  async function reveal() {
    setBusy(true); setError("");
    try { if (record) setBackup(await decryptRoot(record, passphrase)); setPassphrase(""); }
    catch (e) { setError(e instanceof Error ? e.message : "Could not decrypt."); }
    finally { setBusy(false); }
  }

  if (!loaded) return <section className="bettor-page"><p className="section-index">S-06 / PERSONA</p><p role="status" className="t-4 muted">Loading local Persona…</p></section>;
  const registered = info?.registered_height;
  return <section className="bettor-page"><PageHead index="S-06 / THIS DEVICE" title="Persona." kind="crystal" />
    <p className="t-4 muted">A Persona proves a track record without a name. Bets you tag are revealed only if you choose to Thaw. One person may control several Personas.</p>
    {!record && !draft && <div className="ledger-panel"><p className="t-4">No Persona on this device.</p><div className="ledger-actions mt-4"><Button disabled={busy} onClick={create}>{busy ? "Working locally…" : "Create Persona"}</Button></div></div>}
    {draft && <div className="ledger-panel"><h2 className="t-3">01 / Back up your key</h2>
      <p className="t-4 muted mt-2">This 32-byte key cannot be recovered. Lose it and you cannot Thaw. Anyone who sees it can Thaw as you.</p>
      <p className="ledger-code num t-4" aria-label="Backup key">{draft.k_root}</p>
      <div className="ledger-actions"><Button variant="ghost" onClick={() => downloadJson({ persona_id: draft.persona_id, k_root: draft.k_root }, `frosted-persona-${draft.persona_id}.json`)}>Download backup</Button></div>
      <label className="block mt-4"><input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} /> I saved this key somewhere offline.</label>
      <fieldset disabled={busy || !saved} className="builder-inputs"><legend className="t-3">02 / Protect it on this device</legend>
        <label htmlFor="new-passphrase">Passphrase (at least {MIN_PASSPHRASE} characters)<input id="new-passphrase" className="ledger-input" type="password" autoComplete="new-password" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} /></label>
        <label htmlFor="confirm-passphrase">Repeat passphrase<input id="confirm-passphrase" className="ledger-input" type="password" autoComplete="new-password" value={confirm} onChange={(e) => setConfirm(e.target.value)} /></label>
        <div className="ledger-actions"><Button onClick={confirmBackup}>{busy ? "Encrypting…" : "Encrypt and continue"}</Button></div></fieldset></div>}
    {record && <div className="ledger-panel">
      <div className="ledger-title"><h2 className="t-3">Persona <MonoNumber>{record.persona_id.slice(0, 8)}</MonoNumber></h2>
        <StatusBadge status={info ? info.status : "WAITING"} /></div>
      <p role="status" className="num t-4">{registered !== undefined ? <>Registered at block {registered.toLocaleString("en-US")}</> : "Not registered yet. Send the registration payment below."}</p>
      <p className="num t-5 muted">Bets tagged: {tagged} (counted on this device)</p>
      {info?.status === "THAWED" && <p className="t-4 mt-4"><Link className="text-link" href={`/p/${record.persona_id}`}>Open your public profile</Link></p>}
      {registered === undefined && <><h3 className="t-3 mt-6">Registration</h3><p className="t-4 muted">Pay from a shielded wallet. Bets placed before registration are excluded from your record.</p><Qr uri={record.frstp1_uri} /></>}
      <div className="ledger-actions mt-6">
        {info?.status !== "THAWED" && <Link className="text-link t-4" href="/thaw">Thaw this Persona</Link>}
        <Button variant="ghost" onClick={() => downloadJson(record, `frosted-persona-${record.persona_id}-encrypted.json`)}>Export encrypted copy</Button></div>
      <fieldset disabled={busy} className="builder-inputs mt-6"><legend className="t-3">Show backup key</legend>
        <label htmlFor="reveal-passphrase">Passphrase<input id="reveal-passphrase" className="ledger-input" type="password" autoComplete="current-password" value={passphrase} onChange={(e) => setPassphrase(e.target.value)} /></label>
        <div className="ledger-actions"><Button variant="ghost" onClick={reveal}>Show backup key</Button></div>
        {backup && <><p className="ledger-code num t-4" aria-label="Backup key">{backup}</p><Button variant="ghost" onClick={() => setBackup("")}>Hide</Button></>}</fieldset>
    </div>}
    {error && <p role="alert" className="t-4 mt-4">{error}</p>}
  </section>;
}
