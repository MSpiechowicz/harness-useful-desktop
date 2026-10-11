/**
 * The Harness family palette from ../harness-useful-logo (`projects.json`): every product draws the same mark in its
 * own two-stop gradient, `[from (bottom), to (top)]`, so several products stay distinguishable side by side.
 */
export const FAMILY = {
  desktop: ['#38BDF8', '#818CF8'],
  dashboard: ['#F59E0B', '#F472B6'],
  sidebar: ['#8B5CF6', '#C084FC'],
  skills: ['#A3E635', '#22C55E'],
} as const satisfies Record<string, readonly [string, string]>;

export type FamilyProduct = keyof typeof FAMILY;

/** Spoken names of the products, for the marks' text alternatives. */
export const FAMILY_NAMES: Record<FamilyProduct, string> = {
  desktop: 'Harness Desktop',
  dashboard: 'Harness Dashboard',
  sidebar: 'Harness Sidebar',
  skills: 'Harness Skills',
};
