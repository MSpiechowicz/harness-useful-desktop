import type { Action } from 'svelte/action';

/** Moves focus to the node when it mounts, e.g. a step heading so screen readers announce the new step. */
export const autofocus: Action<HTMLElement> = (node) => {
  node.focus({ preventScroll: true });
};
