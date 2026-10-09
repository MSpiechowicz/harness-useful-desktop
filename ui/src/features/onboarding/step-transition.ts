import { cubicOut, expoOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';

import { DURATION, scaled, travel } from '$lib/design/motion';

export interface StepSlideParams {
  /** +1 when moving forward, -1 when moving back. */
  direction: 1 | -1;
}

/**
 * The incoming step only fades its frame in; its pieces then build themselves one after another (the `step-build`
 * cascade in Onboarding.svelte), each gliding in from the side the user is heading towards. Reduced motion
 * crossfades, off is instant.
 */
export function stepIn(_node: Element): TransitionConfig {
  return {
    delay: scaled(140),
    duration: scaled(DURATION.base),
    easing: expoOut,
    css: (t) => `opacity: ${t}`,
  };
}

/** The outgoing step leaves quickly, drifting the opposite way, so the new one has the stage. */
export function stepOut(_node: Element, { direction }: StepSlideParams): TransitionConfig {
  const distance = travel(24) * direction;

  return {
    duration: scaled(180),
    easing: cubicOut,
    css: (t, u) => `opacity: ${t}; transform: translateX(${-u * distance}px)`,
  };
}
