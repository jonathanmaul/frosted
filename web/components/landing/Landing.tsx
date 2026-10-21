"use client";

import gsap from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import Lenis from "lenis";
import { useEffect, useRef, useState } from "react";

import { HeroVisual } from "@/components/landing/HeroVisual";
import { ButtonLink } from "@/components/ui/Button";
import { DecryptedNumber, ScrambledLine } from "@/components/ui/Numbers";
import { OddsBar } from "@/components/ui/OddsBar";
import { formatZec } from "@/lib/api";
import { useReducedMotion } from "@/lib/motion";
import { useFeaturedOdds } from "@/lib/useFeaturedOdds";

const STEPS = [
  { title: "A bet arrives.", body: "A shielded note lands on an Outcome Address. The amount and the outcome are readable with the public viewing key." },
  { title: "The odds are public.", body: "Anyone with the viewing key recomputes every pool. The odds on this page are the same numbers you get from your own node." },
  { title: "The sender stays blurred.", body: "The viewing key shows what came in, never who sent it. No address, no txid, no name appears here." },
  { title: "Payout, 2 of 3.", body: "After resolution, one shielded batch pays the winners. It moves only with 2 of 3 FROST signatures." },
];

export function Landing() {
  const odds = useFeaturedOdds();
  const reduced = useReducedMotion();
  const [step, setStep] = useState(0);
  const stepRefs = useRef<(HTMLDivElement | null)[]>([]);

  // Scrollytelling: Lenis + ScrollTrigger only on the landing page, triggers only in section 02.
  useEffect(() => {
    if (reduced) return;
    gsap.registerPlugin(ScrollTrigger);
    const lenis = new Lenis();
    lenis.on("scroll", ScrollTrigger.update);
    const tick = (time: number) => lenis.raf(time * 1000);
    gsap.ticker.add(tick);
    gsap.ticker.lagSmoothing(0);
    const triggers = stepRefs.current.map((el, i) =>
      ScrollTrigger.create({
        trigger: el,
        start: "top center",
        end: "bottom center",
        onToggle: (self) => {
          if (self.isActive) setStep(i + 1);
        },
        onLeaveBack: () => i === 0 && setStep(0),
      }),
    );
    return () => {
      triggers.forEach((t) => t.kill());
      gsap.ticker.remove(tick);
      lenis.destroy();
    };
  }, [reduced]);

  const pool = formatZec(odds.poolZat);

  return (
    <div className="grid grid-cols-12 gap-x-6">
      {/* 01 — hero: text in columns 1–6 aligned bottom, 3D in columns 7–12 (docs/DESIGN.md §4). */}
      <section className="col-span-12 flex min-h-[78vh] flex-col justify-end border-l border-[var(--border)] py-16 pl-6 md:col-span-6" aria-labelledby="hero-title">
        <p className="section-index mb-6">01 / FROSTED</p>
        <h1 id="hero-title" className="t-1">
          Public odds.
          <br />
          Invisible bettors.
        </h1>
        <p className="t-4 mt-6 max-w-md text-[var(--ice-muted)]">
          <span className="text-[var(--ice)]">{pool === "—" ? "—" : <DecryptedNumber text={pool} />}</span> ZEC in the pool. 0 names.
        </p>
        <div className="mt-8 flex gap-3">
          <ButtonLink href="#markets">Browse markets</ButtonLink>
          <ButtonLink href="/how" variant="ghost">
            How it works
          </ButtonLink>
        </div>
      </section>

      <div className="col-span-12 md:col-span-6 md:row-span-2">
        <div className="md:sticky md:top-20">
          <HeroVisual yes={odds.yes} no={odds.no} step={step} />
          <div className="mt-4 border-t border-[var(--border)] pt-4">
            <p className="t-5 mb-2 text-[var(--ice-muted)]">{odds.market ? odds.market.question : "No open market yet."}</p>
            <OddsBar yes={{ label: "YES", probability: odds.yes }} no={{ label: "NO", probability: odds.no }} />
            <p className="t-5 mt-3 text-[var(--ice-muted)]">
              bettor <ScrambledLine />
            </p>
          </div>
        </div>
      </div>

      {/* 02 — scrollytelling, reusing the same 3D object. */}
      <section className="col-span-12 border-l border-[var(--border)] pl-6 md:col-span-6" aria-labelledby="how-title">
        <p className="section-index mb-6 mt-16">02 / HOW IT WORKS</p>
        <h2 id="how-title" className="t-2 mb-8">
          Four steps. One public key.
        </h2>
        {STEPS.map((s, i) => (
          <div
            key={s.title}
            ref={(el) => {
              stepRefs.current[i] = el;
            }}
            className={`border-t border-[var(--border)] py-16 ${reduced ? "" : "min-h-[60vh]"}`}
          >
            <p className="num t-5 mb-3 text-[var(--ice-muted)]">{String(i + 1).padStart(2, "0")} / 04</p>
            <h3 className="t-3 mb-2">{s.title}</h3>
            <p className="t-4 max-w-md text-[var(--ice-muted)]">{s.body}</p>
          </div>
        ))}
      </section>

      <section id="markets" className="col-span-12 border-t border-[var(--border)] py-16" aria-labelledby="markets-title">
        <p className="section-index mb-4">03 / MARKETS</p>
        <h2 id="markets-title" className="t-2">
          Markets
        </h2>
        <p className="t-4 mt-2 text-[var(--ice-muted)]">The market list arrives with the bettor screens.</p>
      </section>
    </div>
  );
}
