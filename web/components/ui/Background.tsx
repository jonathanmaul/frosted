"use client";

import Noise from "@/components/bits/Noise/Noise";

/** Background grain, opacity ≤ 4% (docs/DESIGN.md §7). */
export function Background() {
  return <Noise fps={0} opacity={0.035} fixed zIndex={0} />;
}
