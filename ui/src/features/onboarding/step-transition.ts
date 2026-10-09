import { cubicOut, expoOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';

import { DURATION, scaled, travel } from '$lib/design/motion';

export interface StepSlideParams {
  /** +1 when moving forward, -1 when moving back. */
  direction: 1 | -1;
}

/** The incoming step slides in from the side the user is heading towards; reduced motion crossfades, off is instant. */
export function stepIn(_node: Element, { direction }: StepSlideParams): TransitionConfig {
  const distance = travel(44) * direction;

  return {
    delay: scaled(100),
    duration: scaled(DURATION.enter),
    easing: expoOut,
    css: (t, u) => `opacity: ${t}; transform: translateX(${u * distance}px)`,
  };
}

/** The outgoing step drifts the opposite way while it fades, so the pair reads as one movement. */
export function stepOut(_node: Element, { direction }: StepSlideParams): TransitionConfig {
  const distance = travel(28) * direction;

  return {
    duration: scaled(DURATION.base),
    easing: cubicOut,
    css: (t, u) => `opacity: ${t}; transform: translateX(${-u * distance}px)`,
  };
}
