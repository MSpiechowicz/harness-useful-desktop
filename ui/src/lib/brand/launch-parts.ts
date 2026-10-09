/** Debris of the launch animation: what falls out of the crack and what the landing kicks up. */

/** A soft dust puff; `slow` ones linger longer. */
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

/**
 * Impact dust: the slab lands flat and drives the air out from under it, so the dust starts beneath the whole
 * block, jets out low and fast along the ground (further at the ends), then slows and billows up at the front.
 * Deliberately uneven: sizes, timing, reach and rise differ, the right side is stronger.
 */
export const PUFFS: readonly Puff[] = [
  // Left jet, from the slab's left end outward.
  { cx: 75, cy: 93.5, r: 5.7, dx: -46, dy: -7, scale: 2.3, delay: 0 },
  { cx: 79, cy: 94, r: 4.6, dx: -33, dy: -4, scale: 1.9, delay: 0.03 },
  { cx: 84, cy: 94, r: 4.1, dx: -19, dy: -10, scale: 1.7, delay: 0.06 },
  // Middle: pushed out less, rises a little.
  { cx: 89, cy: 94, r: 5.1, dx: -5, dy: -12, scale: 1.8, delay: 0.04 },
  { cx: 93, cy: 94, r: 4.3, dx: 8, dy: -9, scale: 1.6, delay: 0.08 },
  // Right jet, the stronger side.
  { cx: 98, cy: 94, r: 5.9, dx: 30, dy: -5, scale: 2.2, delay: 0.01 },
  { cx: 102, cy: 93.5, r: 6.8, dx: 54, dy: -9, scale: 2.6, delay: 0 },
  { cx: 100, cy: 94, r: 4.3, dx: 41, dy: -2, scale: 1.8, delay: 0.05 },
  // Front billows that keep rolling and rising after the jet has spent itself.
  { cx: 76, cy: 93, r: 7.3, dx: -52, dy: -24, scale: 2.6, delay: 0.09, slow: true },
  { cx: 103, cy: 93, r: 8.1, dx: 60, dy: -28, scale: 3, delay: 0.07, slow: true },
  { cx: 88, cy: 93, r: 6.2, dx: 4, dy: -26, scale: 2.4, delay: 0.14, slow: true },
];

/** The dust skirt: a thin, wide sheet that spreads along the ground the instant the slab lands. */
export const SKIRT = { cx: 88.5, cy: 94.5, rx: 18, ry: 2.2 } as const;

/** Small irregular shards, as outlines around their own origin: splinters of the glyph, not pixels. */
export const SHARDS = [
  'M-1.3 -0.9 L1.4 -1.2 L0.6 1.3 Z',
  'M-1 -1.3 L1.2 -0.4 L0.4 1.2 L-1.2 0.5 Z',
  'M-0.8 -1 L1 -0.8 L-0.2 1.1 Z',
] as const;

/** Splinters that hop off the landing; `rise` is the height of the hop (negative is up). */
export interface Chip {
  x: number;
  y: number;
  shard: number;
  scale: number;
  dx: number;
  rise: number;
  delay: number;
}

export const CHIPS: readonly Chip[] = [
  { x: 75, y: 91, shard: 0, scale: 1.2, dx: -20, rise: -15, delay: 0 },
  { x: 78, y: 92, shard: 2, scale: 0.9, dx: -30, rise: -8, delay: 0.04 },
  { x: 102, y: 91, shard: 1, scale: 1.2, dx: 21, rise: -14, delay: 0.01 },
  { x: 100, y: 92, shard: 2, scale: 0.9, dx: 31, rise: -7, delay: 0.05 },
  { x: 89, y: 92, shard: 0, scale: 0.8, dx: -6, rise: -19, delay: 0.03 },
];

/** Grit trickling out of the crack before the break. */
export interface Grit {
  x: number;
  y: number;
  shard: number;
  scale: number;
  dx: number;
  delay: number;
}

export const GRIT: readonly Grit[] = [
  { x: 69.5, y: 65, shard: 2, scale: 0.7, dx: 4, delay: 0 },
  { x: 64.5, y: 65.6, shard: 0, scale: 0.6, dx: -3, delay: 0.12 },
  { x: 61, y: 65.8, shard: 1, scale: 0.65, dx: 2, delay: 0.24 },
];

/** The crack: a thin, sharp line along the fracture, drawn from the outer edge inward; it never leaves the stem. */
export const CRACK = { width: 0.6, d: 'M73 62.9 L71.3 64.6 L69.9 63.8 L67.6 66.3 L66.1 64.9 L64.4 65.6 L62.2 63.7 L60.4 65.9 L58 64.6' } as const;

/** Dust that bursts out of the crack at the snap, drifting sideways and up. */
export const SNAP_PUFFS: readonly Puff[] = [
  { cx: 72, cy: 64, r: 2.6, dx: 9, dy: -5, scale: 2, delay: 0 },
  { cx: 66, cy: 65, r: 2.2, dx: 3, dy: -8, scale: 1.8, delay: 0.04 },
  { cx: 60, cy: 65, r: 2.4, dx: -6, dy: -6, scale: 1.9, delay: 0.02 },
];
