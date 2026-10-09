import type { Action } from 'svelte/action';

import { viewState } from '$lib/stores/view-state.svelte';

const MAX_RESTORE_FRAMES = 30;

/**
 * Restores a scroller to where the user left it and keeps remembering. Content often arrives
 * after mount (lists load), so the saved position is re-applied for a moment, until the user
 * scrolls on their own.
 */
export const scrollMemory: Action<HTMLElement, string> = (node, key) => {
  let current = key;
  let userScrolled = false;
  let frames = 0;
  let frame = 0;
  /** Where our own restore left the scroller; the scroll event it causes is not the user's. */
  let restoredTo: number | null = null;

  function restore(): void {
    const wanted = viewState.scrollOf(current);
    frames += 1;

    if (userScrolled || wanted <= 0) {
      return;
    }

    node.scrollTop = wanted;
    restoredTo = node.scrollTop;

    if (node.scrollTop < wanted && frames < MAX_RESTORE_FRAMES) {
      frame = requestAnimationFrame(restore);
    }
  }

  function onScroll(): void {
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
      current = next;
    },
    destroy() {
      cancelAnimationFrame(frame);
      node.removeEventListener('scroll', onScroll);
    },
  };
};
