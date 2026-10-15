import { flushSync } from 'svelte';
import { describe, expect, it } from 'vitest';

import { DRAWER_BELOW_PX, PaneDrawer } from './pane-drawer.svelte';

/** A drawer made inside an effect root, as a view creates it while it initialises. */
function mount(width: number) {
  let drawer!: PaneDrawer;
  const stop = $effect.root(() => {
    drawer = new PaneDrawer();
    drawer.width = width;
  });
  flushSync();

  return { drawer, stop };
}

describe('the list pane shortcut', () => {
  it('opens and closes the drawer of the view on screen, and does nothing while the list is a column', () => {
    const wide = mount(DRAWER_BELOW_PX + 200);
    expect(PaneDrawer.toggleCurrent()).toBe(false);
    expect(wide.drawer.open).toBe(false);

    const narrow = mount(DRAWER_BELOW_PX - 200);
    expect(PaneDrawer.toggleCurrent()).toBe(true);
    expect(narrow.drawer.open).toBe(true);
    expect(PaneDrawer.toggleCurrent()).toBe(true);
    expect(narrow.drawer.open).toBe(false);

    narrow.stop();
    wide.stop();
    flushSync();
    expect(PaneDrawer.toggleCurrent()).toBe(false);
  });

  it('goes to the newest view when an older one has not been unmounted yet', () => {
    const older = mount(800);
    const newer = mount(800);

    PaneDrawer.toggleCurrent();

    expect(newer.drawer.open).toBe(true);
    expect(older.drawer.open).toBe(false);
    older.stop();
    newer.stop();
  });
});
