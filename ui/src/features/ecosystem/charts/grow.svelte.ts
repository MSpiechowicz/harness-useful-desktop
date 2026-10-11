import { onMount } from 'svelte';

import { motion } from '$lib/stores/motion.svelte';

/**
 * Bars grow from zero once, on the frame after they mount, and only when motion is full (reduced and off keep them
 * still). Call it while the component initialises; read `.grown` in the markup.
 */
export function growOnMount(): { readonly grown: boolean } {
  let grown = $state(motion.level !== 'full');

  onMount(() => {
    if (!grown) {
      const frame = requestAnimationFrame(() => (grown = true));

      return () => cancelAnimationFrame(frame);
    }
  });

  return {
    get grown() {
      return grown;
    },
  };
}
