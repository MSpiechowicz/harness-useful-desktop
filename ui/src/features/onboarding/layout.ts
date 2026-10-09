import type { OnboardingStep } from './onboarding.svelte';

/** Content widths: the cards steps need room to sit side by side, text steps read best narrower. */
export const WIDTH = {
  wide: 'max-w-[60rem]',
  narrow: 'max-w-[38rem]',
} as const;

export const STEP_WIDTH: Record<OnboardingStep, keyof typeof WIDTH> = {
  welcome: 'wide',
  harnesses: 'wide',
  project: 'narrow',
  approval: 'narrow',
  done: 'narrow',
};

/** Brightness of the backdrop aurora per step: loudest on arrival and departure, calmer while choosing. */
export const AURORA_INTENSITY: Record<OnboardingStep, number> = {
  welcome: 0.85,
  harnesses: 0.5,
  project: 0.45,
  approval: 0.4,
  done: 0.8,
};
