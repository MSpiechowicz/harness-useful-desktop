/**
 * The approved "H_" mark on a 128 grid: single outlines, sharp corners. The right-hand leg of the H
 * has fallen over and landed on the baseline as the cursor.
 */

/** The splintered line the right stem breaks along, from its outer edge (x 73) to its inner edge (x 58). */
export const FRACTURE = 'L73 62.9 L71.3 64.6 L69.9 63.8 L67.6 66.3 L66.1 64.9 L64.4 65.6 L62.2 63.7 L60.4 65.9 L58 64.6';

/** The H after the break: the right stem ends in the splintered edge, which it keeps for good. */
export const H_BROKEN = `M18 20 H33 V50 H58 V20 H73 ${FRACTURE} L58 64 H33 V94 H18 Z`;

/** The lower piece of the right stem, between the crack opening and the snap (its top is the same splintered line). */
export const STEM_PIECE = 'M58 64.6 L60.4 65.9 L62.2 63.7 L64.4 65.6 L66.1 64.9 L67.6 66.3 L69.9 63.8 L71.3 64.6 L73 62.9 V94 H58 Z';

/** The intact H, used for the frames before the leg snaps off. */
export const H_INTACT = 'M18 20 H33 V50 H58 V20 H73 V94 H58 V64 H33 V94 H18 Z';

/**
 * The fallen leg, now the cursor: the stem piece turned 90 degrees, so its right end carries the matching
 * splintered edge. It topples about `LEG_PIVOT`.
 */
export const CURSOR = 'M73 79 L102.4 79 L101.1 81.4 L103.3 83.2 L101.4 85.4 L102.1 87.1 L100.7 88.6 L103.2 90.9 L102.4 92.3 L104.1 94 L73 94 Z';
export const LEG_PIVOT = { x: 73, y: 94 } as const;

/** The mark cropped to the glyph bounds, so a name sits flush against it. */
export const GLYPH_BOX = { x: 18, y: 20, width: 86.1, height: 74 } as const;

/** Square canvas with a little padding, for icon slots and the favicon. */
export const ICON_BOX = { x: 13, y: 14.5, width: 95, height: 95 } as const;

/** Gradient axis across the glyph bounds; stops are the app accent, `accent-1 -> accent-2 -> accent-3`. */
export const GRADIENT_AXIS = { x1: 18, y1: 20, x2: 103, y2: 94 } as const;

export const ACCENT_STOPS = [
  { offset: 0, color: 'var(--accent-1)' },
  { offset: 0.55, color: 'var(--accent-2)' },
  { offset: 1, color: 'var(--accent-3)' },
] as const;

export function boxAttribute(box: { x: number; y: number; width: number; height: number }): string {
  return `${box.x} ${box.y} ${box.width} ${box.height}`;
}
