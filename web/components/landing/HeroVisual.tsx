"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";

import { canRender3D } from "@/lib/motion";

const HeroScene = dynamic(() => import("@/components/landing/HeroScene"), { ssr: false });

/** Static placeholder first (server render + fallback); 3D only when the device allows it. */
export function HeroVisual({ yes, no, step }: { yes: number | null; no: number | null; step: number }) {
  const [enable3D, setEnable3D] = useState(false);
  useEffect(() => {
    // Load the 3D bundle only after the page has loaded and the browser is idle, so the text
    // headline stays the LCP element and three.js does not block the main thread (docs/DESIGN.md §10).
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
      if (idle === undefined) return;
      if (hasIdle) window.cancelIdleCallback(idle);
      else window.clearTimeout(idle);
    };
  }, []);
  return (
    <div className="relative aspect-[4/5] w-full">
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src="/hero-static.svg"
        alt="A frosted glass prism holding the odds bars; blurred bettor silhouettes behind it"
        className="absolute inset-0 h-full w-full"
      />
      {enable3D && (
        <div className="absolute inset-0" aria-hidden>
          <HeroScene yes={yes} no={no} step={step} />
        </div>
      )}
    </div>
  );
}
