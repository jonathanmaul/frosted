import { render, screen } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";

import { OddsBar } from "@/components/ui/OddsBar";
import { StatusBadge } from "@/components/ui/StatusBadge";
import { formatPercent, formatZec } from "@/lib/api";
import { LEAKED_METADATA } from "@/lib/content/how";

vi.mock("@/lib/motion", () => ({ useReducedMotion: () => true }));

const css = readFileSync("app/globals.css", "utf8");
const token = (name: string) => css.match(new RegExp(`--${name}:\\s*(#[0-9a-fA-F]{6})`))![1];

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) =>
    c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4,
  );
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
const contrast = (a: string, b: string) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

describe("design tokens (docs/DESIGN.md §2)", () => {
  it("defines the exact palette", () => {
    expect(token("bg").toLowerCase()).toBe("#eef3f8");
    expect(token("ice").toLowerCase()).toBe("#0a1424");
    expect(token("accent").toLowerCase()).toBe("#c93509");
  });
  it("meets WCAG AA for text tokens on the background", () => {
    for (const t of ["ice", "ice-muted", "yes", "no", "accent"]) expect(contrast(token(t), token("bg"))).toBeGreaterThanOrEqual(4.5);
    expect(contrast("#ffffff", token("accent"))).toBeGreaterThanOrEqual(4.5);
  });
  it("caps the radius at 4px", () => {
    expect(css).toMatch(/--radius-2:\s*4px/);
  });
});

describe("base components", () => {
  it("OddsBar shows labels and percentages, not color alone", () => {
    render(<OddsBar yes={{ label: "YES", probability: 0.621 }} no={{ label: "NO", probability: 0.379 }} />);
    expect(screen.getByText("62%")).toBeTruthy();
    expect(screen.getByRole("img").getAttribute("aria-label")).toBe("YES 62%, NO 38%");
  });
  it("OddsBar shows — when odds are unknown", () => {
    render(<OddsBar yes={{ label: "YES", probability: null }} no={{ label: "NO", probability: null }} />);
    expect(screen.getAllByText("—")).toHaveLength(2);
  });
  it("StatusBadge uses §23.2a labels", () => {
    render(<StatusBadge status="REJECTED_TRANSPARENT" />);
    expect(screen.getByText("Rejected, refund")).toBeTruthy();
  });
  it("formats zatoshi without floats", () => {
    expect(formatZec(1_240_000_000)).toBe("12.40");
    expect(formatZec(null)).toBe("—");
    expect(formatPercent(null)).toBe("—");
  });
});

describe("S-12 content", () => {
  it("lists every leaked-metadata item of spec §24.3", () => {
    expect(LEAKED_METADATA).toHaveLength(10);
  });
});
