import type { Action } from 'svelte/action';

/** The container in the shell's top bar that view headers render into. */
export const SHELL_HEADER_SLOT = 'shell-header';

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
