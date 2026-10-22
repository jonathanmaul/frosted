"use client";

// Frosted wrappers around the copied React Bits components (docs/DESIGN.md §7): mono font,
// tokens, and short timing. The copied sources in components/bits/ stay close to upstream.
import { useEffect, useState } from "react";

import CountUp from "@/components/bits/CountUp/CountUp";
import DecryptedText from "@/components/bits/DecryptedText/DecryptedText";
import { useReducedMotion } from "@/lib/motion";

/** A number that rolls from its previous value when a real event changes it. */
export function RollingNumber({ value, decimals = 0, separator = "," }: { value: number | null; decimals?: number; separator?: string }) {
  const reduced = useReducedMotion();
  // Remember the previous value in state (render-time update pattern) so the roll starts from it.
  const [range, setRange] = useState({ from: value ?? 0, to: value });
  if (range.to !== value) setRange({ from: range.to ?? value ?? 0, to: value });
  const from = range.from;
  if (value === null) return <span className="num">—</span>;
  if (reduced) return <span className="num">{value.toLocaleString("en-US", { minimumFractionDigits: decimals, maximumFractionDigits: decimals })}</span>;
  return (
    <CountUp
      key={value}
      from={from}
      to={value}
      duration={0.6}
      decimals={decimals}
      separator={separator}
      trigger="load"
      className="num"
    />
  );
}

/** Odds/pool text that decrypts once into readable numbers. */
export function DecryptedNumber({ text }: { text: string }) {
  const reduced = useReducedMotion();
  if (reduced) return <span className="num">{text}</span>;
  return (
    <DecryptedText
      key={text}
      text={text}
      speed={40}
      maxIterations={12}
      sequential
      characters="0123456789.%"
      animateOn="view"
      className="num"
      encryptedClassName="num text-[var(--ice-muted)]"
    />
  );
}

/** The bettor is always redacted. Scramble only on a real odds event (docs/DESIGN.md §7). */
export function ScrambledLine({ length = 18, eventKey }: { length?: number; eventKey?: string }) {
  const reduced = useReducedMotion();
  const [text, setText] = useState(() => "░▒▓".repeat(Math.ceil(length / 3)).slice(0, length));
  useEffect(() => {
    if (reduced || eventKey === undefined) return;
    const glyphs = "▓▒░#%&@$0123456789abcdef";
    let ticks = 0;
    const id = window.setInterval(() => {
      setText(Array.from({ length }, () => glyphs[Math.floor(Math.random() * glyphs.length)]).join(""));
      if (++ticks === 6) window.clearInterval(id);
    }, 80);
    return () => window.clearInterval(id);
  }, [eventKey, reduced, length]);
  return <span className="num text-[var(--ice-muted)]" aria-label="bettor hidden"><span aria-hidden="true">{text}</span></span>;
}
