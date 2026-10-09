/** Debris of the launch animation: what falls out of the crack and what the landing kicks up. */

/** Soft dust puffs at the impact; `slow` ones linger longer. */
export interface Puff {
  cx: number;
  cy: number;
  r: number;
  dx: number;
  dy: number;
  scale: number;
  delay: number;
  slow?: boolean;
}

export const PUFFS: readonly Puff[] = [
  { cx: 76, cy: 91.5, r: 5, dx: -19, dy: -8, scale: 2.2, delay: 0 },
  { cx: 79, cy: 93, r: 4.2, dx: -28, dy: -2, scale: 1.9, delay: 0.03 },
  { cx: 87, cy: 94, r: 4.6, dx: -5, dy: -13, scale: 2.1, delay: 0.02 },
  { cx: 94, cy: 94, r: 4.6, dx: 6, dy: -12, scale: 2.0, delay: 0.04 },
  { cx: 99, cy: 93, r: 4.2, dx: 27, dy: -3, scale: 1.9, delay: 0.02 },
  { cx: 101, cy: 91.5, r: 5, dx: 19, dy: -9, scale: 2.3, delay: 0 },
  { cx: 82, cy: 92, r: 6.5, dx: -22, dy: -14, scale: 2.3, delay: 0.08, slow: true },
  { cx: 96, cy: 92, r: 6.5, dx: 22, dy: -15, scale: 2.4, delay: 0.1, slow: true },
];

/** Pixel chips that hop off the landing; `rise` is the height of the hop (negative is up). */
export interface Chip {
  x: number;
  y: number;
  size: number;
  dx: number;
  rise: number;
  delay: number;
}

export const CHIPS: readonly Chip[] = [
  { x: 74, y: 90, size: 2.6, dx: -20, rise: -15, delay: 0 },
  { x: 77, y: 91, size: 2, dx: -30, rise: -8, delay: 0.04 },
  { x: 101, y: 90, size: 2.6, dx: 21, rise: -14, delay: 0.01 },
  { x: 99, y: 91, size: 2, dx: 31, rise: -7, delay: 0.05 },
  { x: 88, y: 91, size: 2, dx: -6, rise: -19, delay: 0.03 },
];

/** Grit trickling out of the crack before the break. */
export interface Grit {
  x: number;
  y: number;
  size: number;
  dx: number;
  delay: number;
}

export const GRIT: readonly Grit[] = [
  { x: 60, y: 64, size: 1.8, dx: -5, delay: 0 },
  { x: 66, y: 65, size: 1.5, dx: 3, delay: 0.06 },
  { x: 70, y: 64, size: 2, dx: 7, delay: 0.11 },
];

/** The crack across the right stem at the joint, and its side branch. */
export const CRACK = {
  main: { width: 1.8, d: 'M57.5 63.2 L60.6 65.4 L62.9 62.6 L65.8 66.1 L68.4 63.4 L70.6 65.6 L73.5 64.2' },
  branch: { width: 1.1, d: 'M65.8 66.1 L66.9 69.2 M62.9 62.6 L62.2 59.4' },
} as const;
