import { DRAWER_BELOW_PX } from '$lib/design/layout';
import { focusQuietly } from '$lib/utils/focus-quietly';

/**
 * The state of a list pane that turns into a drawer on narrow windows. Create it while a component initialises:
 * `WorkspaceState` owns one, `WorkspaceLayout` binds `width` and shows it in a `ListDrawer`, `WorkspaceToggles` flips `open`.
 */
export class PaneDrawer {
  /** The width of the area the list and detail share; 0 until it is measured. */
  width = $state(0);
  open = $state(false);
  /** Whether the list is a drawer at the current width. */
  readonly active = $derived(Number.isFinite(this.width) && this.width > 0 && this.width < DRAWER_BELOW_PX);

  /** @param toggleSelector Gives the selector of the button that opens the drawer, so `dismiss` hands focus back to that one when other views are on screen. */
  constructor(private readonly toggleSelector: () => string = () => '[data-drawer-toggle]') {
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
    focusQuietly(document.querySelector<HTMLElement>(this.toggleSelector()));
  }
}
