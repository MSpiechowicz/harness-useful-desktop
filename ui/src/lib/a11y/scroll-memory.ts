import type { Action } from 'svelte/action';

import { viewState } from '$lib/stores/view-state.svelte';

const MAX_RESTORE_FRAMES = 30;

/**
 * Restores a scroller to where the user left it and keeps remembering. Content often arrives
 * after mount (lists load), so the saved position is re-applied for a moment, until the user
 * scrolls on their own. When the key changes (another item in the same scroller), the scroller
 * goes to the position saved for the new key, or to the top if it has none. Without a key (`undefined`) it
 * does nothing, for a scroller that is only sometimes the page's.
 */
export const scrollMemory: Action<HTMLElement, string | undefined> = (node, key) => {
  let current = key;
  let userScrolled = false;
  let frames = 0;
  let frame = 0;
  /** Where our own restore left the scroller; the scroll event it causes is not the user's. */
  let restoredTo: number | null = null;

  function restore(): void {
    if (current === undefined) {
      return;
    }

    const wanted = viewState.scrollOf(current);
    frames += 1;

    if (userScrolled) {
      return;
    }

    if (wanted <= 0) {
      if (node.scrollTop > 0) {
        node.scrollTop = 0;
        restoredTo = node.scrollTop;
      }

      return;
    }

    node.scrollTop = wanted;
    restoredTo = node.scrollTop;

    if (node.scrollTop < wanted && frames < MAX_RESTORE_FRAMES) {
      frame = requestAnimationFrame(restore);
    }
  }

  function onScroll(): void {
    if (current === undefined) {
      return;
    }

    if (restoredTo !== null && Math.abs(node.scrollTop - restoredTo) <= 1) {
      restoredTo = null;

      return;
    }

    userScrolled = true;
    viewState.rememberScroll(current, node.scrollTop);
  }

  frame = requestAnimationFrame(restore);
  node.addEventListener('scroll', onScroll, { passive: true });

  return {
    update(next) {
      if (next === current) {
        return;
      }

      // The old key already holds the user's last position (onScroll saves it), so switching
      // only has to start over for the new one: its saved position, or the top.
      cancelAnimationFrame(frame);
      current = next;
      userScrolled = false;
      restoredTo = null;
      frames = 0;
      restore();
    },
    destroy() {
      cancelAnimationFrame(frame);
      node.removeEventListener('scroll', onScroll);
    },
  };
};
