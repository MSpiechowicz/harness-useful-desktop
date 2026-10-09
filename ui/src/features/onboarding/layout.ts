import type { OnboardingStep } from './onboarding.svelte';

/** Content width of the steps (the frame width), so every step lines up with the next. */
export const WIDTH = {
  wide: 'max-w-[60rem]',
} as const;

/** Every step uses the full frame width, so titles start at the same place from step to step. */
export const STEP_WIDTH: Record<OnboardingStep, keyof typeof WIDTH> = {
  welcome: 'wide',
  harnesses: 'wide',
  project: 'wide',
  approval: 'wide',
  done: 'wide',
};

/** Brightness of the backdrop aurora per step: loudest on arrival and departure, calmer while choosing. */
export const AURORA_INTENSITY: Record<OnboardingStep, number> = {
  welcome: 0.85,
  harnesses: 0.5,
  project: 0.45,
  approval: 0.4,
  done: 0.8,
};
