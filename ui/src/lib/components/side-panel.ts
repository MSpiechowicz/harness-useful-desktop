/** How wide a `SidePanel` is; a view that makes room for the panel beside its table uses the same length. The token is `--side-panel`. */
export const SIDE_PANEL_WIDTH = 'var(--side-panel)';

/** What `--side-panel` is where nothing styles the page (tests, a browser preview before the stylesheet loads). */
const FALLBACK_PX = 448;

/** `--side-panel` as a number, for the places that need a size in script (the starting width of a draggable list). */
export function sidePanelPx(): number {
  const value = Number.parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--side-panel'));

  return Number.isFinite(value) && value > 0 ? value : FALLBACK_PX;
}
