import type { Metadata } from "next";

import { LEAKED_METADATA, LEGAL, PRIVATE_ITEMS, PUBLIC_ITEMS, TRUST_LIMITS } from "@/lib/content/how";

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
    <div className="py-16">
      <header className="grid grid-cols-12 gap-x-6 pb-12">
        <p className="section-index col-span-12 mb-6">S-12 / HOW IT WORKS</p>
        <h1 className="t-1 col-span-12 md:col-span-9">The Glass Escrow.</h1>
        <p className="t-4 col-span-12 mt-6 max-w-2xl text-[var(--ice-muted)] md:col-span-7">
          Each market is a shielded Zcash account whose incoming viewing key is published. Anyone can see what
          came in. Nobody can see who sent it.
        </p>
        <div className="col-span-12 mt-10 grid grid-cols-12 gap-x-6 border-t border-[var(--border)] pt-6 md:col-span-9">
          <div className="col-span-4">
            <p className="num t-5 text-[var(--ice-muted)]">BETTOR</p>
            <p className="t-4">Sends a shielded note with a sealed memo.</p>
          </div>
          <div className="col-span-4">
            <p className="num t-5 text-[var(--ice-muted)]">ANYONE WITH THE UIVK</p>
            <p className="t-4">Sees amounts, outcomes, memos. Recomputes the odds.</p>
          </div>
          <div className="col-span-4">
            <p className="num t-5 text-[var(--ice-muted)]">NOBODY</p>
            <p className="t-4">Sees the sender.</p>
          </div>
        </div>
      </header>
      <Section index="01 / PUBLIC" title="What anyone can see" items={PUBLIC_ITEMS} />
      <Section index="02 / PRIVATE" title="What stays private" items={PRIVATE_ITEMS} />
      <Section index="03 / TRUST" title="What you still trust" items={TRUST_LIMITS} />
      <Section index="04 / METADATA" title="Metadata that still leaks" items={LEAKED_METADATA} />
      <section className="grid grid-cols-12 gap-x-6 border-t border-[var(--border)] py-12">
        <p className="section-index col-span-12 mb-4 md:col-span-3 md:mb-0">05 / LEGAL</p>
        <p className="t-4 col-span-12 text-[var(--ice-muted)] md:col-span-8">{LEGAL}</p>
      </section>
    </div>
  );
}
