import type { Action } from 'svelte/action';

/**
 * The three places in the shell's top bar that a view header renders into, left to right.
 * Each swaps on its own terms when the view changes: the title crossfades in place, the controls fade one after the other,
 * and the fixed end (layout toggles and the "⋯" menu) does not animate at all, so it looks like it never changed.
 */
export const HEADER_TITLE_SLOT = 'shell-header-title';
export const HEADER_CONTROLS_SLOT = 'shell-header-controls';
export const HEADER_FIXED_SLOT = 'shell-header-fixed';

/**
 * Moves the element into the container with this id, so a view can render its header inside the shell's top bar.
 * Without such a container (a view rendered on its own, as in tests) the element stays where it is.
 * The element is removed on destroy: it no longer lives inside the block that owned it.
 */
export const portal: Action<HTMLElement, string> = (node, targetId) => {
  document.getElementById(targetId)?.append(node);

  return {
    destroy() {
      node.remove();
    },
  };
};
