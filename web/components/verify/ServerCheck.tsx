"use client";

// The server-side check, labelled as untrustworthy (docs/DESIGN.md: no decorative colour, text labels carry meaning).
import { useState } from "react";
import { Button } from "@/components/ui/Button";
import { fetchServerCheck, formatDiffValue, type VerifyResult } from "@/lib/verify";

export function ServerCheck({ marketId }: { marketId: string }) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<VerifyResult | null>(null);
  async function run() {
    setBusy(true); setResult(null);
    try { setResult(await fetchServerCheck(marketId)); } finally { setBusy(false); }
  }
  return <section className="ledger-panel" aria-labelledby="server-check-title">
    <h2 id="server-check-title" className="t-3">Server-side check</h2>
    <p className="t-4 muted mt-2">For convenience only. Do not trust it: a dishonest server could print PASS. Run the command above against your own node for an independent result.</p>
    <div className="ledger-actions mt-4"><Button variant="ghost" disabled={busy} onClick={run}>{busy ? "Checking against the chain…" : "Run server-side check"}</Button></div>
    <div role="status" aria-live="polite">
      {busy && <p className="t-4 muted mt-4">The server scans its node from the market&apos;s birthday block. This can take a minute.</p>}
      {result && !result.ok && <p role="alert" className="t-4 mt-4">{result.message}</p>}
      {result?.ok && <div className="mt-6">
        <p className="num t-5 muted">Server node at block {result.report.node_tip.toLocaleString("en-US")}{result.report.api_height !== null && <>, API data at block {result.report.api_height.toLocaleString("en-US")}</>}</p>
        <ul className="verify-list">{result.report.checks.map((c) => <li key={c.id} className="verify-row">
          <p className="num t-5"><span className="verify-status">[{c.status}]</span> {c.name}</p>
          <p className="t-5 muted">{c.detail}</p>
          {c.diff && <dl className="verify-diff num t-5"><div><dt>Verifier computed</dt><dd>{formatDiffValue(c.diff.verifier)}</dd></div><div><dt>API says</dt><dd>{formatDiffValue(c.diff.api)}</dd></div></dl>}
        </li>)}</ul>
        <p className="t-4 mt-6">Result: <strong>{result.report.ok ? "PASS" : "FAIL"}</strong>. {result.report.trust}</p>
      </div>}
    </div>
  </section>;
}
