// Through the Frost camera path, normalized to section 02 (docs/DESIGN.md §13).
export type JourneyState = { progress: number; active: boolean };
type Point = readonly [number, number, number];
const SHOTS: { at: number; position: Point; target: Point }[] = [
  { at: 0, position: [0, 0.1, 6.5], target: [0, 0, 0] },
  { at: 0.16, position: [0, 0.08, 1.6], target: [0, 0, -4] },
  { at: 0.32, position: [-0.6, 0.3, -3.8], target: [-1.3, 0, -10] },
  { at: 0.52, position: [0.8, 0.45, -9], target: [-0.9, 0.1, -16] },
  { at: 0.73, position: [-0.45, 0.2, -15], target: [-1.2, 0, -22] },
  { at: 0.9, position: [0.1, 0.7, -20.5], target: [-0.7, 0, -27] },
  { at: 1, position: [0, 1.2, -28], target: [0, 0, -36] },
];

export function journeyFrame(value: number): { position: Point; target: Point } {
  const progress = Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0;
  const index = SHOTS.findIndex((shot) => shot.at >= progress);
  const to = SHOTS[Math.max(1, index)];
  const from = SHOTS[Math.max(0, index - 1)];
  const t = (progress - from.at) / (to.at - from.at);
  const eased = t * t * (3 - 2 * t);
  const mix = (a: Point, b: Point): Point => [a[0] + (b[0] - a[0]) * eased, a[1] + (b[1] - a[1]) * eased, a[2] + (b[2] - a[2]) * eased];
  return { position: mix(from.position, to.position), target: mix(from.target, to.target) };
}

export function journeyChapter(progress: number): number {
  return Math.min(3, Math.max(-1, Math.floor(((progress - 0.18) / 0.82) * 4)));
}
