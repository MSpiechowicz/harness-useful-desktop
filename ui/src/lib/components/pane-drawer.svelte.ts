/** Below this width a list pane moves into a drawer so the detail beside it keeps its room. */
export const DRAWER_BELOW_PX = 1100;

/**
 * The state of a list pane that turns into a drawer on narrow windows. Create it while a component initialises:
 * `ListDetail` (or a view's own container) binds `width`, `DrawerToggle` flips `open`, `ListDrawer` shows it.
 */
export class PaneDrawer {
  /** The width of the area the list and detail share; 0 until it is measured. */
  width = $state(0);
  open = $state(false);
  /** Whether the list is a drawer at the current width. */
  readonly active = $derived(this.width > 0 && this.width < DRAWER_BELOW_PX);

  constructor() {
    // Widening the window past the breakpoint turns the drawer back into a column.
    $effect(() => {
      if (!this.active) {
        this.open = false;
      }
    });
  }

  toggle(): void {
    this.open = !this.open;
  }

  /** Puts the drawer away, e.g. after something was chosen in it. */
  close(): void {
    this.open = false;
  }

  /** Puts the drawer away and hands focus back to its toggle (Escape, a click on the scrim). */
  dismiss(): void {
    this.open = false;
    document.querySelector<HTMLElement>('[data-drawer-toggle]')?.focus();
  }
}
