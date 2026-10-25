"use client";

// Layout, copy, and motion follow docs/DESIGN.md.
import Link from "next/link";
import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";

import { HeroVisual } from "@/components/landing/HeroVisual";
import { ButtonLink } from "@/components/ui/Button";
import { Scene3D } from "@/components/ui/Scene3D";
import { DecryptedNumber } from "@/components/ui/Numbers";
import { OddsBar } from "@/components/ui/OddsBar";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { formatZec } from "@/lib/api";
import { journeyChapter, type JourneyState } from "@/lib/journey";
import { useReducedMotion } from "@/lib/motion";
import { useFeaturedOdds } from "@/lib/useFeaturedOdds";

const STEPS = [
  { title: "A bet goes in.", body: "Choose an outcome. Send a shielded Zcash note. Your payout address is sealed in your browser.", detail: "INPUT / SHIELDED NOTE" },
  { title: "The odds come into view.", body: "Amounts are counted. Pools update. A public viewing key lets anyone check the same numbers.", detail: "OUTPUT / PUBLIC ODDS" },
  { title: "The bettor stays invisible.", body: "The glass reveals the bet, never its sender. No account. No public wallet. No name on the ledger.", detail: "IDENTITY / NOT DISCLOSED" },
  { title: "The outcome settles it.", body: "A result is verified. A shielded payout batch moves with 2 of 3 FROST signatures. The coordinator knows payout addresses.", detail: "SETTLEMENT / 2 OF 3 SIGNATURES" },
];

export function Landing({ markets }: { markets?: ReactNode }) {
  const odds = useFeaturedOdds();
  const reduced = useReducedMotion();
  const [step, setStep] = useState(0);
  const [sceneReady, setSceneReady] = useState(false);
  const landing = useRef<HTMLDivElement>(null);
  const story = useRef<HTMLElement>(null);
  const frame = useRef<HTMLDivElement>(null);
  const prologue = useRef<HTMLDivElement>(null);
  const specimen = useRef<HTMLDivElement>(null);
  const journey = useRef<JourneyState>({ progress: 0, active: false });
  const chapterStops = useRef<number[]>([]);
  const stepRefs = useRef<(HTMLDivElement | null)[]>([]);

  // The opening hides the menu; once the visitor reaches the markets the full top bar comes back.
  useEffect(() => {
    const markets = document.getElementById("markets");
    const root = landing.current;
    if (!markets || !root) return;
    const update = () => root.toggleAttribute("data-nav", markets.getBoundingClientRect().top < window.innerHeight * 0.6);
    update();
    window.addEventListener("scroll", update, { passive: true });
    window.addEventListener("resize", update);
    return () => { window.removeEventListener("scroll", update); window.removeEventListener("resize", update); root.removeAttribute("data-nav"); };
  }, []);

  useEffect(() => {
    if (reduced || !sceneReady) return;
    let disposed = false;
    let cleanup = () => {};
    void Promise.all([import("gsap"), import("gsap/ScrollTrigger"), import("lenis")]).then(([{ default: gsap }, { ScrollTrigger }, { default: Lenis }]) => {
      if (disposed || !story.current || !frame.current) return;
      gsap.registerPlugin(ScrollTrigger);
      const media = gsap.matchMedia();
      media.add({ desktop: "(min-width: 900px) and (min-height: 600px)", compact: "(max-width: 899px) and (min-height: 600px)", short: "(max-height: 599px)" }, (context) => {
        const desktop = Boolean(context.conditions?.desktop);
        const cinematic = desktop || Boolean(context.conditions?.compact);
        const surface = specimen.current?.querySelector<HTMLElement>(".scene-viewport");
        const stage = specimen.current?.querySelector<HTMLElement>(".specimen-stage");
        if (!surface || !stage) return;
        const panels = [...stepRefs.current];
        const opening = prologue.current;
        const heading = frame.current?.querySelector<HTMLElement>(".journey-heading");
        const footer = frame.current?.querySelector<HTMLElement>(".journey-footer");
        const playhead = { progress: 0 };
        let inJourney = false;
        let currentChapter = -1;
        landing.current?.toggleAttribute("data-cinematic", cinematic);
        const draw = () => {
          const p = playhead.progress;
          journey.current.progress = p;
          journey.current.active = inJourney;
          if (cinematic) {
            landing.current?.toggleAttribute("data-travelling", inJourney);
            if (inJourney) {
              Object.assign(surface.style, { opacity: String(Math.min(1, (1 - p) / 0.07)) });
            } else surface.removeAttribute("style");
            if (opening) {
              const fade = Math.max(0, 1 - p / 0.14);
              Object.assign(opening.style, { opacity: String(fade), visibility: fade > 0 ? "visible" : "hidden", transform: `translateY(${-p * 180}px) scale(${1 - p * 0.35})` });
              opening.inert = p > 0.14;
              opening.setAttribute("aria-hidden", String(p > 0.14));
            }
            if (footer) { footer.inert = p < 0.18; footer.setAttribute("aria-hidden", String(p < 0.18)); }
            if (heading) heading.style.opacity = String(Math.min(1, Math.max(0, (p - 0.14) / 0.05)));
            frame.current?.style.setProperty("--journey-progress", String(p));
            frame.current?.style.setProperty("--world-reveal", String(Math.min(1, Math.max(0, (p - 0.14) / 0.05))));
            panels.forEach((el, i) => {
              if (!el) return;
              const visible = i === journeyChapter(p);
              el.inert = !visible;
              el.setAttribute("aria-hidden", String(!visible));
              const local = ((p - 0.18) / 0.82) * 4 - i;
              const fadeIn = Math.min(1, Math.max(0, local / 0.12));
              const fadeOut = i === 3 ? 1 : Math.min(1, Math.max(0, (1 - local) / 0.12));
              const opacity = visible ? fadeIn * fadeOut : 0;
              Object.assign(el.style, { opacity: String(opacity), visibility: visible ? "visible" : "hidden", transform: `translateY(${(1 - opacity) * (local > 0.5 ? -18 : 18)}px)` });
            });
          }
          const chapter = inJourney ? journeyChapter(p) : -1;
          if (chapter !== currentChapter) { currentChapter = chapter; setStep(chapter + 1); }
        };
        const timeline = gsap.to(playhead, {
          progress: 1, ease: "none", onUpdate: draw,
          scrollTrigger: {
            trigger: story.current, pin: cinematic ? frame.current : false,
            start: cinematic ? "top top" : "top 70%",
            end: cinematic ? () => `+=${window.innerHeight * (desktop ? 4.4 : 3)}` : "bottom 25%",
            scrub: cinematic ? 0.65 : 0.35, invalidateOnRefresh: true,
            onToggle: (self) => { inJourney = self.isActive; draw(); },
            onRefresh: (self) => {
              chapterStops.current = [0.27, 0.48, 0.68, 0.89].map((p) => self.start + (self.end - self.start) * p);
            },
          },
        });
        const lenis = new Lenis({ autoRaf: false, virtualScroll: () => {
          // Process native input too, so Lenis cancels leftover momentum when leaving the pin.
          lenis.options.smoothWheel = inJourney;
          return true;
        } });
        lenis.on("scroll", ScrollTrigger.update);
        const tick = (time: number) => lenis.raf(time * 1000);
        gsap.ticker.add(tick);
        draw();
        return () => {
          timeline.scrollTrigger?.kill(); timeline.kill();
          gsap.ticker.remove(tick); lenis.destroy();
          journey.current = { progress: 0, active: false };
          surface.removeAttribute("style");
          if (opening) { opening.removeAttribute("style"); opening.inert = false; opening.removeAttribute("aria-hidden"); }
          heading?.removeAttribute("style");
          if (footer) { footer.inert = false; footer.removeAttribute("aria-hidden"); }
          landing.current?.removeAttribute("data-cinematic");
          landing.current?.removeAttribute("data-travelling");
          frame.current?.style.removeProperty("--journey-progress");
          frame.current?.style.removeProperty("--world-reveal");
          panels.forEach((el) => { if (el) { el.inert = false; el.removeAttribute("aria-hidden"); el.removeAttribute("style"); } });
        };
      });
      const anchor = window.location.hash;
      const restoreAnchor = window.requestAnimationFrame(() => {
        if (anchor === "#markets" || anchor === "#how-it-works") document.getElementById(anchor.slice(1))?.scrollIntoView();
      });
      cleanup = () => { window.cancelAnimationFrame(restoreAnchor); media.revert(); };
    }).catch(() => { /* Keep the complete static narrative if scroll enhancement cannot load. */ });
    return () => { disposed = true; cleanup(); };
  }, [reduced, sceneReady]);

  return (
    <div ref={landing} className="landing">
      <section ref={story} id="how-it-works" className="frost-experience" aria-label="Through the Frost">
        <div ref={frame} className="journey-frame">
          <div ref={specimen} className="journey-specimen"><HeroVisual yes={odds.yes} no={odds.no} step={reduced ? 0 : step} journey={journey} onAvailabilityChange={setSceneReady} /></div>
          <div ref={prologue} className="journey-prologue">
            <p className="section-index">A PUBLIC LEDGER. A PRIVATE WORLD.</p>
            <h1 id="hero-title" className="t-1">Public odds.{" "}<br /><em>Private people.</em></h1>
            <p className="t-4">The numbers are yours to read.<br />The people are not.</p>
            <a href="#first-chapter" className="enter-frost num t-5" onClick={(event) => { const top = chapterStops.current[0]; if (top !== undefined) { event.preventDefault(); window.scrollTo({ top, behavior: "instant" }); } }}>SCROLL TO ENTER <span aria-hidden="true">↓</span></a>
          </div>
          <div className="journey-heading"><p className="section-index">THROUGH THE FROST</p><h2 id="how-title" className="t-2">Read what stays visible.</h2></div>
          <div className="journey-steps" id="first-chapter">
            {STEPS.map((s, i) => (
              <div key={s.title} ref={(el) => { stepRefs.current[i] = el; }} className={`story-step ${step === i + 1 ? "is-current" : ""}`}>
                <span className="step-number num">{String(i + 1).padStart(2, "0")}</span>
                <div><p className="section-index mb-3">{s.detail}</p><h3 className="t-3 mb-4">{s.title}</h3><p className="t-4 text-[var(--ice-muted)]">{s.body}</p></div>
              </div>
            ))}
          </div>
          <div className="journey-footer">
            <div className="journey-chapters" role="group" aria-label="Journey chapters">{STEPS.map((s, i) => <button key={s.title} type="button" aria-label={`Go to chapter ${i + 1}: ${s.title}`} aria-pressed={step === i + 1} onClick={() => { const top = chapterStops.current[i]; if (top !== undefined) window.scrollTo({ top, behavior: "instant" }); }} className="num t-5"><span>{String(i + 1).padStart(2, "0")}</span><span>{["ENTER", "VERIFY", "SHIELD", "SETTLE"][i]}</span></button>)}</div>
            <Link className="text-link t-4" href="/how">Full mechanism & trust limits <span aria-hidden="true">↗</span></Link>
          </div>
          <div className="journey-progress" aria-hidden="true" />
        </div>
      </section>
      <section id="markets" className="market-section" aria-labelledby="markets-title">
        <div className="section-topline"><p className="section-index">03 / THE PUBLIC LEDGER</p><span className="num t-5 text-[var(--ice-muted)]">AMOUNTS VISIBLE. SENDERS SHIELDED.</span></div>
        <div className="market-section-heading"><h2 id="markets-title" className="t-2">A position. <em>Without a profile.</em></h2><span className="num t-5 text-[var(--ice-muted)]">FEATURED MARKET</span></div>
        {odds.market ? (
          <article className="featured-market">
            <div><StatusBadge status={odds.market.status} /><h3 className="t-3 mt-5">{odds.market.question}</h3><p className="num t-5 mt-4 text-[var(--ice-muted)]">CLOSES AT BLOCK {odds.market.close_height.toLocaleString("en-US")}</p></div>
            <div><OddsBar yes={{ label: "YES", probability: odds.yes }} no={{ label: "NO", probability: odds.no }} /><p className="num t-5 mt-5"><DecryptedNumber text={formatZec(odds.poolZat)} /> ZEC IN THE POOL</p></div>
            <ButtonLink variant="ghost" href={`/markets/${encodeURIComponent(odds.market.market_id)}`}>View market <span aria-hidden="true">↗</span></ButtonLink>
          </article>
        ) : (
          <div className="market-empty"><span className="num t-5">— / —</span><div><h3 className="t-3">No open market available.</h3><p className="t-4 text-[var(--ice-muted)] mt-2">The ledger is quiet. Live markets appear here when the public API is available.</p></div><span className="section-index">AWAITING DATA</span></div>
        )}
        <p className="market-note num t-5">PUBLIC VIEWING KEY / INDEPENDENTLY VERIFIABLE / NO SENDER IDENTITIES</p>
        {markets}
      </section>
      <section className="closing-note band"><p className="section-index">THE FROSTED PRINCIPLE</p><div><h2 className="t-2">The numbers tell the story.<br /><em>The people don’t have to.</em></h2><Link href="/how" className="text-link t-4 mt-8">Know what’s public. Know what isn’t. <span aria-hidden="true">↗</span></Link></div><Scene3D kind="crystal" className="band-scene" /></section>
    </div>
  );
}
