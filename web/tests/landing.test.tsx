import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { Landing } from "@/components/landing/Landing";
import HowPage from "@/app/how/page";
import { LEAKED_METADATA } from "@/lib/content/how";

const { useOdds } = vi.hoisted(() => ({ useOdds: vi.fn() }));
vi.mock("@/lib/useFeaturedOdds", () => ({ useFeaturedOdds: useOdds }));
vi.mock("@/lib/motion", () => ({ useReducedMotion: () => true }));
vi.mock("@/components/landing/HeroVisual", () => ({ HeroVisual: () => <div data-testid="specimen" /> }));
afterEach(cleanup);

describe("frozen ledger landing", () => {
  it("keeps the mechanism readable and never invents market data while offline", () => {
    useOdds.mockReturnValue({ market: null, yes: null, no: null, poolZat: null });
    render(<Landing />);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Public odds. Invisible bettors.");
    expect(screen.queryByRole("link", { name: /Skip to markets/ })).toBeNull();
    expect(screen.getByText("No open market available.")).toBeTruthy();
    expect(screen.getByText("A bet goes in.")).toBeTruthy();
    expect(screen.getByText("The outcome settles it.")).toBeTruthy();
  });

  it("shows actual featured odds and links to the selected market", () => {
    useOdds.mockReturnValue({ market: { market_id: "test-market", question: "Will the threshold be reached?", status: "OPEN", close_height: 1234 }, yes: 0.62, no: 0.38, poolZat: 1_240_000_000 });
    render(<Landing />);
    expect(screen.getByText("Will the threshold be reached?")).toBeTruthy();
    expect(screen.getByRole("link", { name: "View market" }).getAttribute("href")).toBe("/markets/test-market");
    expect(screen.getAllByRole("img", { name: "YES 62%, NO 38%" })).toHaveLength(1);
    expect(screen.getByText("CLOSES AT BLOCK 1,234")).toBeTruthy();
  });

  it("renders all ten metadata disclosures and the legal notice on S-12", () => {
    render(<HowPage />);
    for (const disclosure of LEAKED_METADATA) expect(screen.getByText(disclosure)).toBeTruthy();
    expect(screen.getByText(/Frosted is research and a demo/)).toBeTruthy();
    expect(screen.getByText(/This demo has one person holding all committee shares/)).toBeTruthy();
  });
});
