import type { OnboardingStep } from './onboarding-state.svelte';

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
