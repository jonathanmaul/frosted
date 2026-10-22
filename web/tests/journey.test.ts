import { describe, expect, it } from "vitest";
import { journeyChapter, journeyFrame } from "@/lib/journey";

describe("Through the Frost camera", () => {
  it("starts outside the original specimen, crosses the glass, and reaches the payout", () => {
    expect(journeyFrame(0).position).toEqual([0, 0.1, 6.5]);
    expect(journeyFrame(0.32).position[2]).toBeLessThan(0);
    expect(journeyFrame(1).position[2]).toBe(-28);
  });
  it("moves continuously forward and can reverse to the same shot", () => {
    let previousZ = Infinity;
    for (let i = 0; i <= 1000; i++) {
      const shot = journeyFrame(i / 1000);
      expect(shot.position[2]).toBeLessThanOrEqual(previousZ);
      expect(shot.target[2]).toBeLessThan(shot.position[2]);
      expect([...shot.position, ...shot.target].every(Number.isFinite)).toBe(true);
      previousZ = shot.position[2];
    }
    const shot = journeyFrame(0.48);
    journeyFrame(0.95);
    expect(journeyFrame(0.48)).toEqual(shot);
  });
  it("clamps overscroll and maps all four chapters including the endpoint", () => {
    expect(journeyFrame(-1)).toEqual(journeyFrame(0));
    expect(journeyFrame(2)).toEqual(journeyFrame(1));
    expect(journeyFrame(NaN)).toEqual(journeyFrame(0));
    expect([0, 0.18, 0.4, 0.6, 0.8, 1].map(journeyChapter)).toEqual([-1, 0, 1, 2, 3, 3]);
  });
});
