import { Spring } from 'svelte/motion';
import { cubicOut, expoOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';

import { motion } from '$lib/stores/motion.svelte';

/** Millisecond durations mirroring the CSS tokens in motion.css. */
export const DURATION = { fast: 120, base: 200, slow: 320, enter: 480 } as const;

/** Scales a duration to the effective motion level: full as-is, reduced shortened, off zero. */
export function scaled(ms: number): number {
  if (motion.level === 'off') {
    return 0;
  }
  if (motion.level === 'reduced') {
    return Math.min(ms, DURATION.fast);
  }

  return ms;
}

/** Travel distance for entering surfaces; reduced motion crossfades in place. */
export function travel(px: number): number {
  return motion.level === 'full' ? px : 0;
}

/** Damped-spring easing (about 6% overshoot) as a pure function of progress. */
export function springEase(t: number): number {
  if (t <= 0 || t >= 1) {
    return t <= 0 ? 0 : 1;
  }

  return 1 - Math.exp(-7 * t) * Math.cos(11 * t);
}

export interface RiseParams {
  y?: number;
  duration?: number;
  delay?: number;
}

/** Fade in/out with a small vertical travel. */
export function rise(_node: Element, { y = 8, duration = DURATION.base, delay = 0 }: RiseParams = {}): TransitionConfig {
  const distance = travel(y);

  return {
    delay,
    duration: scaled(duration),
    easing: expoOut,
    css: (t, u) => `opacity: ${t}; transform: translateY(${u * distance}px)`,
  };
}

export interface PopParams {
  from?: number;
  duration?: number;
  /** Spring overshoot on the way in; disable for outros. */
  spring?: boolean;
}

/** Scale + fade used by dialogs and popovers; springs in, eases out. */
export function pop(_node: Element, { from = 0.94, duration = DURATION.slow, spring = true }: PopParams = {}): TransitionConfig {
  const amount = motion.level === 'full' ? 1 - from : 0;

  return {
    duration: scaled(duration),
    easing: spring ? springEase : cubicOut,
    css: (t, u) => `opacity: ${Math.min(1, t * 2)}; transform: scale(${1 - u * amount})`,
  };
}

/** Spring with the repo's tuning; callers move it with `moveSpring` so the motion setting is honoured. */
export function createSpring(initial: number): Spring<number> {
  return new Spring(initial, { stiffness: 0.16, damping: 0.55 });
}

export function moveSpring(spring: Spring<number>, target: number): void {
  if (motion.level === 'full') {
    spring.target = target;
  } else {
    spring.set(target, { instant: true });
  }
}
