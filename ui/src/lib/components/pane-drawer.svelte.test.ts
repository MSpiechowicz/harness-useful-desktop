import { flushSync } from 'svelte';
import { describe, expect, it } from 'vitest';

import { DRAWER_BELOW_PX } from '$lib/design/layout';

import { PaneDrawer } from './pane-drawer.svelte';

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

describe('the list drawer', () => {
  it('is a drawer only once the width is measured and below the breakpoint', () => {
    const unmeasured = mount(0);
    const wide = mount(DRAWER_BELOW_PX);
    const narrow = mount(DRAWER_BELOW_PX - 1);

    expect(unmeasured.drawer.active).toBe(false);
    expect(wide.drawer.active).toBe(false);
    expect(narrow.drawer.active).toBe(true);

    unmeasured.stop();
    wide.stop();
    narrow.stop();
  });

  it('puts itself away when the window grows wide enough for a column', () => {
    const view = mount(DRAWER_BELOW_PX - 200);
    view.drawer.toggle();
    expect(view.drawer.open).toBe(true);

    view.drawer.width = DRAWER_BELOW_PX + 200;
    flushSync();

    expect(view.drawer.open).toBe(false);
    view.stop();
  });
});
