import { DETAILS_INLINE_FROM_PX } from '$lib/design/layout';
import { focusQuietly } from '$lib/utils/focus-quietly';
import { readStored, writeStored } from '$lib/utils/persisted';

import { PaneDrawer } from './pane-drawer.svelte';
import { sidePanelPx } from './side-panel';

export type WorkspaceId = 'sessions' | 'changes' | 'config' | 'extensions';

type Panel = 'list' | 'details';

/** Only a column that was hidden on a wide window is remembered. A drawer or an overlay always starts closed. */
function storageKey(id: WorkspaceId, panel: Panel): string {
  return `harness.workspace.${id}.${panel}`;
}

/**
 * The state of a view laid out as list | centre | details (Sessions, Changes, Config, Extensions).
 *
 * Wide, the list and the details are columns the user shows or hides, and the choice is remembered. Narrower, the
 * list becomes a drawer and the details a slide-over, both closed until asked for. Create it while a component
 * initialises: `WorkspaceLayout` takes it as `workspace`, `WorkspaceToggles` flips it, the shortcuts reach it through
 * `toggleCurrentList` and `toggleCurrentDetails`.
 */
export class WorkspaceState {
  /** The layouts on screen, the newest last: a view stays mounted while the next one crossfades in over it. */
  private static readonly onScreen: WorkspaceState[] = [];

  readonly id: WorkspaceId;
  /** Whether the view has a details panel at all (Changes has none). */
  readonly hasDetails: boolean;
  /** The list as a drawer on narrow windows; it also holds the measured width of the view. */
  readonly drawer = new PaneDrawer(() => `[data-workspace="${this.id}"][data-drawer-toggle]`);

  /** The width in px of the list column. Kept here so hiding and showing the column keeps a dragged width. */
  listSize = $state(sidePanelPx());
  /** Whether the list column shows while there is room for it. */
  listShown = $state(true);
  /** Whether the details column shows while there is room for it. */
  detailsShown = $state(true);
  /** Whether the details slide-over is open; only meaningful when the details are not a column. */
  overlayOpen = $state(false);
  /** Whether a `WorkspaceLayout` of this state is on screen; the toggles show nothing before. */
  attached = $state(false);

  /** The list is a column (not a drawer); true until the view is measured. */
  readonly listInline = $derived(!this.drawer.active);
  /** The details are a column (not a slide-over); true until the view is measured. */
  readonly detailsInline = $derived.by(() => this.hasDetails && (!this.measured || this.width >= DETAILS_INLINE_FROM_PX));
  /** Whether the list can be seen: the column is shown, or the drawer is open. */
  readonly listVisible = $derived(this.listInline ? this.listShown : this.drawer.open);
  /** Whether the details can be seen: the column is shown, or the slide-over is open. */
  readonly detailsVisible = $derived.by(() => this.hasDetails && (this.detailsInline ? this.detailsShown : this.overlayOpen));

  constructor(id: WorkspaceId, { details }: { details: boolean }) {
    this.id = id;
    this.hasDetails = details;
    this.listShown = readStored(storageKey(id, 'list')) !== 'false';
    this.detailsShown = readStored(storageKey(id, 'details')) !== 'false';

    // Widening the window until the details have a column turns the slide-over back into it.
    $effect(() => {
      if (this.detailsInline) {
        this.overlayOpen = false;
      }
    });
  }

  /** Whether the view has a usable width: anything else (0, NaN, undefined) is unmeasured and counts as wide. */
  private get measured(): boolean {
    return Number.isFinite(this.width) && this.width > 0;
  }

  /** The width of the view; 0 until it is measured, which counts as wide. */
  get width(): number {
    return this.drawer.width;
  }

  set width(value: number) {
    this.drawer.width = value;
  }

  /** Marks this state as the one on screen while a layout is mounted; the returned function undoes it. */
  attach(): () => void {
    WorkspaceState.onScreen.push(this);
    this.attached = true;

    return () => {
      const index = WorkspaceState.onScreen.indexOf(this);
      if (index >= 0) {
        WorkspaceState.onScreen.splice(index, 1);
      }

      this.attached = WorkspaceState.onScreen.includes(this);
    };
  }

  /** The shortcut: shows or hides the list of the view on screen; false when no view is laid out. */
  static toggleCurrentList(): boolean {
    const current = WorkspaceState.onScreen.at(-1);
    if (!current) {
      return false;
    }

    current.toggleList();

    return true;
  }

  /** The shortcut: shows or hides the details of the view on screen; false when no view is laid out or it has none. */
  static toggleCurrentDetails(): boolean {
    const current = WorkspaceState.onScreen.at(-1);
    if (!current?.hasDetails) {
      return false;
    }

    current.toggleDetails();

    return true;
  }

  /** Hides or shows the list column, or opens or puts away the drawer. */
  toggleList(): void {
    if (!this.listInline) {
      if (this.drawer.open) {
        this.drawer.dismiss();
      } else {
        this.drawer.open = true;
      }

      return;
    }

    if (this.listShown) {
      this.yieldFocus('list');
    }

    this.listShown = !this.listShown;
    writeStored(storageKey(this.id, 'list'), String(this.listShown));
  }

  /** Hides or shows the details column, or opens or puts away the slide-over. */
  toggleDetails(): void {
    if (!this.hasDetails) {
      return;
    }

    if (!this.detailsInline) {
      if (this.overlayOpen) {
        this.closeOverlay();
      } else {
        this.overlayOpen = true;
      }

      return;
    }

    if (this.detailsShown) {
      this.yieldFocus('details');
    }

    this.detailsShown = !this.detailsShown;
    writeStored(storageKey(this.id, 'details'), String(this.detailsShown));
  }

  /** The close button of the details: hides the column or puts the slide-over away, whichever is showing. */
  closeDetails(): void {
    if (this.detailsVisible) {
      this.toggleDetails();
    }
  }

  /** Puts the details slide-over away (Escape); focus goes to its toggle only if it was inside the panel. */
  closeOverlay(): void {
    if (!this.overlayOpen) {
      return;
    }

    this.yieldFocus('details');
    this.overlayOpen = false;
  }

  /** Before a panel goes, focus that sits inside it moves to its toggle instead of falling to the page. */
  private yieldFocus(panel: Panel): void {
    const owner = `[data-workspace="${this.id}"]`;
    const inside = document.activeElement?.closest(`${owner}[data-workspace-panel="${panel}"]`);
    if (!inside) {
      return;
    }

    const toggle = panel === 'list' ? '[data-drawer-toggle]' : '[data-details-toggle]';
    focusQuietly(document.querySelector<HTMLElement>(`${owner}${toggle}`));
  }
}
