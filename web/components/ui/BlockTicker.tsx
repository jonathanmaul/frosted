"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";

import { RollingNumber } from "@/components/ui/Numbers";
import { fetchTip } from "@/lib/api";

const POLL_MS = 5_000;

/** Header block-height ticker. Ticks only when a new block height arrives. */
export function BlockTicker() {
  const pathname = usePathname();
  const [height, setHeight] = useState<number | null>(null);
  useEffect(() => {
    // The local WASM harness must not initiate API requests (NFR-P2).
    if (pathname === "/wasm-test") return;
    let alive = true;
    const poll = async () => {
      const h = await fetchTip();
      if (alive) setHeight(h);
    };
    poll();
    const id = window.setInterval(poll, POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, [pathname]);
  return (
    <span className="t-5 text-[var(--ice-muted)]" aria-live="polite">
      <span className="num mr-2">BLOCK</span>
      <span className="text-[var(--ice)]">
        <RollingNumber value={height} />
      </span>
    </span>
  );
}
