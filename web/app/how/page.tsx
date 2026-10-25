import type { Metadata } from "next";

import { ButtonLink } from "@/components/ui/Button";
import { PageHead } from "@/components/ui/Scene3D";
import { BUILT_ON, LEAKED_METADATA, LEGAL, PRIVATE_ITEMS, PUBLIC_ITEMS, TRUST_LIMITS } from "@/lib/content/how";

export const metadata: Metadata = { title: "How it works · Frosted" };

function Section({ index, title, items }: { index: string; title: string; items: string[] }) {
  return (
    <section className="grid grid-cols-12 gap-x-6 border-t border-[var(--border)] py-12">
      <p className="section-index col-span-12 mb-4 md:col-span-3 md:mb-0">{index}</p>
      <div className="col-span-12 md:col-span-8">
        <h2 className="t-2 mb-6">{title}</h2>
        <ul className="space-y-3">
          {items.map((item) => (
            <li key={item} className="t-4 border-l border-[var(--border)] pl-4 text-[var(--ice-muted)]">
              {item}
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

export default function HowPage() {
  return (
    // S-12 follows docs/DESIGN.md and preserves every disclosure in spec §24.3.
    <div className="py-12 md:py-20">
      <header className="grid grid-cols-12 gap-x-6 pb-12">
        <div className="col-span-12"><PageHead big index="S-12 / THE MECHANISM" title={<>The Glass Escrow.<br /><em className="text-[var(--ice-muted)]">Read the numbers.</em></>} kind="crystal" lede="Each market is a shielded Zcash account whose incoming viewing key is published. Anyone can see what came in. Nobody can see who sent it." /></div>
        <div className="col-span-12 mt-8"><ButtonLink href="/#markets" variant="ghost">Explore the public ledger <span aria-hidden="true">↗</span></ButtonLink></div>
        <figure className="col-span-12">
          <div className="escrow-diagram">
            <div className="diagram-node"><p className="section-index">01 / INPUT</p><h2 className="t-3">Shielded note</h2><p className="t-4 text-[var(--ice-muted)]">An amount. An outcome.<br />A sealed payout address.</p></div>
            <span className="diagram-arrow" aria-hidden="true">→</span>
            <div className="diagram-node"><p className="section-index">02 / GLASS ESCROW</p><h2 className="t-3">Public viewing key</h2><p className="t-4 text-[var(--ice-muted)]">Amounts and outcomes are readable. The sender stays shielded.</p></div>
            <span className="diagram-arrow" aria-hidden="true">→</span>
            <div className="diagram-node"><p className="section-index">03 / SETTLEMENT</p><h2 className="t-3">Shielded payout</h2><p className="t-4 text-[var(--ice-muted)]"><span className="num">2 of 3</span> FROST signatures.<br />One payout batch.</p></div>
          </div>
          <figcaption className="num t-5 mt-4 text-[var(--ice-muted)]">FIG. 01 / THE GLASS SHOWS WHAT CAME IN. NEVER WHO SENT IT.</figcaption>
        </figure>
      </header>
      <Section index="01 / PUBLIC" title="What anyone can see" items={PUBLIC_ITEMS} />
      <Section index="02 / PRIVATE" title="What stays private" items={PRIVATE_ITEMS} />
      <Section index="03 / TRUST" title="What you still trust" items={TRUST_LIMITS} />
      <Section index="04 / METADATA" title="Metadata that still leaks" items={LEAKED_METADATA} />
      <section className="grid grid-cols-12 gap-x-6 border-t border-[var(--border)] py-12">
        <p className="section-index col-span-12 mb-4 md:col-span-3 md:mb-0">05 / LEGAL</p>
        <div className="col-span-12 md:col-span-8">
          <h2 className="t-2 mb-6">Before using Frosted</h2>
          <details className="border-y border-[var(--border)] py-5" open><summary className="t-3 cursor-pointer">Is Frosted a betting service?</summary><p className="t-4 mt-4 text-[var(--ice-muted)]">{LEGAL}</p></details>
          <details className="border-b border-[var(--border)] py-5"><summary className="t-3 cursor-pointer">Is every part of a bet private?</summary><p className="t-4 mt-4 text-[var(--ice-muted)]">No. Amounts, outcomes, block heights and memos are readable with the public viewing key. The coordinator knows payout addresses. Thaw makes a Persona’s tagged bets public and linked.</p></details>
          <details className="border-b border-[var(--border)] py-5"><summary className="t-3 cursor-pointer">Can the committee withhold funds?</summary><p className="t-4 mt-4 text-[var(--ice-muted)]">Yes. FROST requires multiple signatures, but there is no Zcash L1 payout script forcing the committee to pay. This deployment does not provide independent custody.</p></details>
        </div>
      </section>
      <section className="grid grid-cols-12 gap-x-6 border-t border-[var(--border)] py-12" aria-labelledby="built-on-title">
        <p className="section-index col-span-12 mb-4 md:col-span-3 md:mb-0">06 / BUILT ON</p>
        <div className="col-span-12 md:col-span-9">
          <h2 id="built-on-title" className="t-2 mb-6">The Zcash stack underneath</h2>
          <ul className="built-on">
            {BUILT_ON.map((item) => (
              <li key={item.name}>
                <a href={item.href} target="_blank" rel="noopener noreferrer">
                  <span className="num t-3">{item.name} <span aria-hidden="true">↗</span></span>
                  <span className="t-4 text-[var(--ice-muted)]">{item.role}</span>
                </a>
              </li>
            ))}
          </ul>
          <p className="t-5 mt-6 text-[var(--ice-muted)]">Frosted is an independent project, not affiliated with Electric Coin Co. or the Zcash Foundation.</p>
        </div>
      </section>
    </div>
  );
}
