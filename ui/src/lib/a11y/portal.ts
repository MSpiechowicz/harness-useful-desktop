import type { Action } from 'svelte/action';

/** Moves the node under `target` (default `document.body`) so overlays escape clipping and stacking contexts. */
export const portal: Action<HTMLElement, HTMLElement | undefined> = (node, target = document.body) => {
  target.appendChild(node);

  return {
    destroy() {
      node.remove();
    },
  };
};
