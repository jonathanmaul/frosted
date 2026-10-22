"use client";

import dynamic from "next/dynamic";
import { Component, useEffect, useRef, useState, type ReactNode, type RefObject } from "react";

import type { JourneyState } from "@/lib/journey";
import { canRender3D, useReducedMotion } from "@/lib/motion";

const HeroScene = dynamic(() => import("@/components/landing/HeroScene"), { ssr: false });

class SceneBoundary extends Component<{ children: ReactNode; onFailure: () => void }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch() { this.props.onFailure(); }
  render() { return this.state.failed ? null : this.props.children; }
}

/** Static image is present from SSR; no 3D work blocks the headline (docs/DESIGN.md §10). */
export function HeroVisual({ yes, no, step, journey, onAvailabilityChange }: { yes: number | null; no: number | null; step: number; journey: RefObject<JourneyState>; onAvailabilityChange: (ready: boolean) => void }) {
  const [enable3D, setEnable3D] = useState(false);
  const [active, setActive] = useState(true);
  const [ready, setReady] = useState(false);
  const reduced = useReducedMotion();
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const hasIdle = typeof window.requestIdleCallback === "function";
    let idle: number | undefined;
    const start = () => {
      const run = () => setEnable3D(canRender3D());
      idle = hasIdle ? window.requestIdleCallback(run, { timeout: 3000 }) : window.setTimeout(run, 1500);
    };
    if (document.readyState === "complete") start();
    else window.addEventListener("load", start, { once: true });
    return () => {
      window.removeEventListener("load", start);
      if (idle !== undefined) { if (hasIdle) window.cancelIdleCallback(idle); else window.clearTimeout(idle); }
    };
  }, []);
  useEffect(() => {
    const observer = new IntersectionObserver(([entry]) => setActive(entry.isIntersecting));
    if (root.current) observer.observe(root.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    onAvailabilityChange(ready && enable3D && !reduced);
  }, [ready, enable3D, reduced, onAvailabilityChange]);
  return <div ref={root} className="hero-visual">
    <div className="specimen-stage">
      <div className="scene-viewport">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img src="/models/frost-ledger-static.webp" width="800" height="1000" alt="A faceted ice ledger with suspended folios, etched calibration marks, and frosted odds bars." className="specimen-image" />
        {enable3D && !reduced && <SceneBoundary onFailure={() => setEnable3D(false)}><div className="specimen-canvas" aria-hidden="true"><HeroScene yes={yes} no={no} step={step} journey={journey} active={active || step > 0} onReady={() => setReady(true)} onFailure={() => setEnable3D(false)} /></div></SceneBoundary>}
      </div>
    </div>
  </div>;
}
