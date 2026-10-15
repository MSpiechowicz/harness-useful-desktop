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
  setup: 'wide',
  done: 'wide',
};

/** The columns of a card's header line: the mark, the name and the state chip, which ends at the card's edge. */
export const SETUP_COLUMNS = 'grid grid-cols-[var(--control-h)_minmax(0,1fr)_auto] items-center gap-x-2.5';

/**
 * The columns of the harness matrix, one grid for its header and every row: the harness (whatever room is left), one
 * column of equal width per product (a state chip), and the column of the
 * row's own disclosure. Rows of any number of harnesses line up because they all use this.
 */
export const SETUP_MATRIX = 'grid grid-cols-[minmax(0,1fr)_200px_200px_var(--control-h)] items-center gap-x-2.5';
