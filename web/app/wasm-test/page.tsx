"use client";

import { useState } from "react";
import { Button } from "@/components/ui/Button";
import { loadGlassWasm } from "@/lib/wasm";

// Test harness follows docs/DESIGN.md; secrets and raw memos never enter the DOM.
export default function WasmTestPage() {
  const [input, setInput] = useState("");
  const [status, setStatus] = useState("Ready. Paste a test BuildBet JSON object. No payment will be sent.");
  const [busy, setBusy] = useState(false);
  async function run() {
    setBusy(true);
    try {
      const wasm = await loadGlassWasm();
      const result = wasm.build_bet(JSON.parse(input));
      if (!result.memo_text.startsWith("FRST1:") || result.memo_text.length !== 237 || !result.zip321_uri.startsWith("zcash:")) throw new Error("Output check failed");
      setStatus(`PASS. 237 memo bytes. Fingerprint ${result.bet_fingerprint}.`);
    } catch {
      setStatus("FAIL. Check the test input and run pnpm build:wasm before retrying.");
    } finally { setBusy(false); }
  }
  return <section className="mx-auto max-w-3xl px-6 py-24">
    <p className="t-5 font-mono text-[var(--ice-muted)]">M3 / LOCAL WASM CHECK</p>
    <h1 className="t-2 my-6">Browser bet builder.</h1>
    <label className="t-4 block" htmlFor="wasm-input">Test input JSON</label>
    <textarea id="wasm-input" value={input} onChange={(event) => setInput(event.target.value)} rows={12} spellCheck={false} className="t-5 my-4 w-full rounded-[2px] border border-[var(--border)] bg-[var(--surface)] p-4 font-mono" />
    <Button onClick={run} disabled={busy || !input.trim()}>{busy ? "Checking…" : "Run local check"}</Button>
    <p role="status" className="t-4 mt-6">{status}</p>
  </section>;
}
