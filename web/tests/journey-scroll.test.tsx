import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useEffect } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Landing } from "@/components/landing/Landing";

type ScrollConfig = {
  onUpdate: () => void;
  scrollTrigger: {
    pin: HTMLElement | false;
    onToggle: (value: { isActive: boolean }) => void;
    onRefresh: (value: { start: number; end: number }) => void;
  };
};
const state = vi.hoisted(() => ({
  playhead: null as { progress: number } | null,
  config: null as ScrollConfig | null,
  teardown: null as (() => void) | null,
  destroy: vi.fn(),
  kill: vi.fn(),
  reduced: false,
  lenisOptions: null as { smoothWheel?: boolean; virtualScroll: () => boolean | void } | null,
}));
vi.mock("@/lib/motion", () => ({ useReducedMotion: () => state.reduced }));
vi.mock("@/lib/useFeaturedOdds", () => ({ useFeaturedOdds: () => ({ market: null, yes: null, no: null, poolZat: null }) }));
vi.mock("@/components/ui/Numbers", () => ({ DecryptedNumber: ({ text }: { text: string }) => <span>{text}</span>, RollingNumber: () => <span>—</span>, ScrambledLine: () => <span>░▒▓</span> }));
vi.mock("@/components/landing/HeroVisual", () => ({ HeroVisual: ({ onAvailabilityChange }: { onAvailabilityChange: (ready: boolean) => void }) => {
  useEffect(() => onAvailabilityChange(true), [onAvailabilityChange]);
  return <div className="specimen-stage"><div className="scene-viewport" data-testid="surface" /></div>;
} }));
vi.mock("gsap", () => ({ default: {
  registerPlugin: vi.fn(),
  matchMedia: () => ({ add: (_queries: unknown, callback: (context: { conditions: { desktop: boolean } }) => () => void) => { state.teardown = callback({ conditions: { desktop: true } }); }, revert: () => state.teardown?.() }),
  to: (playhead: { progress: number }, config: ScrollConfig) => { state.playhead = playhead; state.config = config; return { kill: state.kill, scrollTrigger: { kill: state.kill } }; },
  ticker: { add: vi.fn(), remove: vi.fn() },
} }));
vi.mock("gsap/ScrollTrigger", () => ({ ScrollTrigger: { update: vi.fn() } }));
vi.mock("lenis", () => ({ default: class {
  constructor(public options: { smoothWheel?: boolean; virtualScroll: () => boolean | void }) { state.lenisOptions = options; }
  on() {} raf() {} destroy() { state.destroy(); }
} }));
afterEach(() => { cleanup(); state.config = null; state.playhead = null; state.lenisOptions = null; state.reduced = false; vi.restoreAllMocks(); });

function progress(value: number) {
  act(() => {
    state.playhead!.progress = value;
    state.config!.scrollTrigger.onToggle({ isActive: true });
    state.config!.onUpdate();
  });
}

describe("scroll journey enhancement", () => {
  it("hands scroll back to the browser without bypassing Lenis momentum cancellation", async () => {
    render(<Landing />);
    await waitFor(() => expect(state.lenisOptions).not.toBeNull());
    progress(0.89);
    expect(state.lenisOptions!.virtualScroll()).toBe(true);
    expect(state.lenisOptions!.smoothWheel).toBe(true);
    act(() => state.config!.scrollTrigger.onToggle({ isActive: false }));
    // Returning false here leaves an old smooth animation running against native wheel input.
    expect(state.lenisOptions!.virtualScroll()).toBe(true);
    expect(state.lenisOptions!.smoothWheel).toBe(false);
    progress(0.75);
    state.lenisOptions!.virtualScroll();
    expect(state.lenisOptions!.smoothWheel).toBe(true);
  });

  it("pins only after the scene is ready, supports reversing and keyboard chapter jumps, and cleans up", async () => {
    const scroll = vi.spyOn(window, "scrollTo").mockImplementation(() => {});
    const { container, unmount } = render(<Landing />);
    await waitFor(() => expect(state.config).not.toBeNull());
    expect(state.config!.scrollTrigger.pin).toBe(container.querySelector(".journey-frame"));
    const surface = screen.getByTestId("surface");
    const steps = Array.from(container.querySelectorAll<HTMLElement>(".story-step"));
    progress(0.6);
    expect(Number(surface.style.opacity)).toBeGreaterThan(0);
    expect(steps.map((el) => el.inert)).toEqual([true, true, false, true]);
    expect(screen.getByRole("button", { name: /Go to chapter 3/ }).getAttribute("aria-pressed")).toBe("true");
    progress(0.27);
    expect(steps.map((el) => el.getAttribute("aria-hidden"))).toEqual(["false", "true", "true", "true"]);
    act(() => state.config!.scrollTrigger.onRefresh({ start: 100, end: 1100 }));
    fireEvent.click(screen.getByRole("button", { name: /Go to chapter 4/ }));
    expect(scroll).toHaveBeenCalledWith({ top: 990, behavior: "instant" });
    unmount();
    expect(surface.getAttribute("style")).toBeNull();
    expect(steps.every((el) => !el.inert && !el.hasAttribute("aria-hidden"))).toBe(true);
    expect(state.destroy).toHaveBeenCalled();
  });

  it("keeps all four steps readable with reduced motion", () => {
    state.reduced = true;
    const { container } = render(<Landing />);
    expect(container.querySelector("[data-cinematic]")).toBeNull();
    expect(screen.getAllByRole("heading", { level: 3 })).toHaveLength(5);
    expect(container.querySelectorAll('[aria-hidden="true"] .story-step')).toHaveLength(0);
    expect(state.config).toBeNull();
  });
});
