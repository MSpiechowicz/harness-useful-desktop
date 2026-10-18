import type { TransitionConfig } from 'svelte/transition';

import { DURATION, scaled } from './motion';

/**
 * Set on the parts of a view header that are on their way out of the top bar. Svelte keeps them in the slot until
 * their transition ends, while the view that replaces them is already there: the slots use the mark to decide which
 * part to show (see TopBar), and the transitions below use it to find out whether a replacement exists yet.
 */
export const LEAVING_ATTRIBUTE = 'data-leaving';

/**
 * Set on a fixed part a moment after it appears. Its buttons do not all attach at once (the layout toggles come with
 * the view, the "⋯" menu with its data), so the part the user sees stays until the next one has had this long to
 * fill in, and is not swapped for a half-built one.
 */
export const READY_ATTRIBUTE = 'data-ready';

const followed = new WeakSet<Element>();

/**
 * Marks the node as leaving, and keeps the mark equal to whether Svelte holds it inert. Svelte makes a node inert for
 * the length of its outro and restores it when the view comes back while the node is still on its way out (the same
 * node is then shown again, and its second outro does not run the transition function a second time). The mark has to
 * follow both ways, or a node that came back would stay "leaving" for good.
 */
function markLeaving(node: Element): void {
  node.setAttribute(LEAVING_ATTRIBUTE, '');

  if (followed.has(node)) {
    return;
  }

  followed.add(node);
  new MutationObserver(() => {
    const inert = node.hasAttribute('inert');

    node.toggleAttribute(LEAVING_ATTRIBUTE, inert);
  }).observe(node, { attributes: true, attributeFilter: ['inert'] });
}

function replacementExists(node: Element): boolean {
  return node.parentElement?.querySelector(`:scope > :not([${LEAVING_ATTRIBUTE}])`) != null;
}

/** Marks the node ready once it has had `DURATION.fast` to fill in. */
export function readyAfterMount(node: Element): { destroy: () => void } {
  const wait = scaled(DURATION.fast);

  if (wait === 0) {
    node.setAttribute(READY_ATTRIBUTE, '');

    return { destroy: () => {} };
  }

  const timer = setTimeout(() => node.setAttribute(READY_ATTRIBUTE, ''), wait);

  return { destroy: () => clearTimeout(timer) };
}

/**
 * For the fixed end of a header (layout toggles, "⋯"): looks like nothing at all. It exists so that Svelte treats the
 * part as leaving, which makes it inert from the first moment (the part sits in the top bar, outside the view that is
 * being swapped out, so it would stay clickable until it is removed) and so that the part carries the leaving mark.
 */
export function fixedOut(node: Element): TransitionConfig {
  markLeaving(node);

  return { duration: scaled(DURATION.base) };
}

/**
 * For the title of a header: the outgoing title stays fully visible until the incoming one exists, and only then
 * fades, over the same time the incoming one fades in. The two sum to one the whole way, so a view that loads late
 * never leaves a dim or empty bar. If no replacement shows up within a while it fades anyway.
 */
export function titleOut(node: Element): TransitionConfig {
  markLeaving(node);

  const fade = scaled(DURATION.base);
  const patience = scaled(DURATION.slow);
  const total = patience + fade;
  let fadeStart: number | null = null;
  let lastElapsed = 0;

  return {
    duration: total,
    tick: (_remaining, elapsedShare) => {
      const elapsed = elapsedShare * total;
      const element = node as HTMLElement;

      // A title that came back and leaves again starts over: the first outro's moment of fading no longer applies.
      if (elapsed < lastElapsed) {
        fadeStart = null;
      }

      lastElapsed = elapsed;

      if (fadeStart === null && (elapsed >= patience || replacementExists(node))) {
        fadeStart = elapsed;
      }

      let progress = 0;

      if (fadeStart !== null) {
        // With motion off there is no fade to divide by: the outgoing title is simply gone.
        progress = fade === 0 ? 1 : Math.min(1, (elapsed - fadeStart) / fade);
      }

      element.style.opacity = String(1 - progress);
      element.style.visibility = progress >= 1 ? 'hidden' : '';
    },
  };
}
