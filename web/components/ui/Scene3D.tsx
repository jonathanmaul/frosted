"use client";

import dynamic from "next/dynamic";
import { Component, useEffect, useRef, useState, type ReactNode } from "react";

import { canRender3D } from "@/lib/motion";

import type { SceneKind } from "./scenes";

const Scene = dynamic(() => import("./scenes"), { ssr: false });

class Boundary extends Component<{ children: ReactNode; onFailure: () => void }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch() { this.props.onFailure(); }
  render() { return this.state.failed ? null : this.props.children; }
}

/** Decorative 3D for page headers. Renders nothing without WebGL or under reduced motion; never carries information alone. */
export function Scene3D({ kind, yes, no, className = "" }: { kind: SceneKind; yes?: number | null; no?: number | null; className?: string }) {
  const [enabled, setEnabled] = useState(false);
  const [active, setActive] = useState(true);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => { const id = window.setTimeout(() => { try { setEnabled(canRender3D()); } catch { setEnabled(false); } }, 300); return () => window.clearTimeout(id); }, []);
  useEffect(() => {
    if (typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver(([entry]) => setActive(entry.isIntersecting));
    if (root.current) observer.observe(root.current);
    return () => observer.disconnect();
  }, []);
  return <div ref={root} className={`scene3d ${className}`} aria-hidden="true">
    {enabled && <Boundary onFailure={() => setEnabled(false)}><Scene kind={kind} active={active} yes={yes} no={no} onFailure={() => setEnabled(false)} /></Boundary>}
  </div>;
}

/** Page header: text on the left, a small 3D scene on the right (docs/DESIGN.md §14). */
export function PageHead({ index, title, lede, kind, yes, no, big, children }: { big?: boolean; index: string; title: ReactNode; lede?: ReactNode; kind: SceneKind; yes?: number | null; no?: number | null; children?: ReactNode }) {
  return <header className="page-head">
    <div className="page-head-copy"><p className="section-index">{index}</p><h1 className={`${big ? "t-1" : "t-2"} page-title`}>{title}</h1>{lede && <p className="t-4 muted page-lede">{lede}</p>}{children}</div>
    <Scene3D kind={kind} yes={yes} no={no} />
  </header>;
}
