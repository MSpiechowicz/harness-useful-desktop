/**
 * The approved "H_" mark on a 128 grid: single outlines, sharp corners. The right-hand leg of the H
 * has fallen over and landed on the baseline as the cursor.
 */

/** The H after the break (lower-right leg gone). */
export const H_BROKEN = 'M18 20 H33 V50 H58 V20 H73 V64 H33 V94 H18 Z';

/** The intact H, used for the frames before the leg snaps off. */
export const H_INTACT = 'M18 20 H33 V50 H58 V20 H73 V94 H58 V64 H33 V94 H18 Z';

/** The fallen leg, now the cursor. It topples 90 degrees about `LEG_PIVOT`. */
export const CURSOR = { x: 73, y: 79, width: 30, height: 15 } as const;
export const LEG_PIVOT = { x: 73, y: 94 } as const;

/** The mark cropped to the glyph bounds, so a name sits flush against it. */
export const GLYPH_BOX = { x: 18, y: 20, width: 85, height: 74 } as const;

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
