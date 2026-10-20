import type { OnboardingStep } from './onboarding-state.svelte';

/** Content width of the steps (the frame width), so every step lines up with the next. */
export const WIDTH = {
  wide: 'max-w-[60rem]',
} as const;

/** Every step uses the full frame width, so titles start at the same place from step to step. */
export const STEP_WIDTH: Record<OnboardingStep, keyof typeof WIDTH> = {
  welcome: 'wide',
  meet: 'wide',
  harnesses: 'wide',
  project: 'wide',
  approval: 'wide',
  family: 'wide',
  setup: 'wide',
  done: 'wide',
};

/** The columns of a card's header line: the mark, the name and the state chip, which ends at the card's edge. */
export const SETUP_COLUMNS = 'grid grid-cols-[var(--control-h)_minmax(0,1fr)_auto] items-center gap-x-2.5';

/**
 * The columns of the harness matrix, one grid for its header and every row: the harness (whatever room is left), one
 * column of equal width per MCP server (a state chip), and the column of the row's own disclosure. Rows of any number
 * of harnesses line up because they all use this. Each server's column is wide enough for its title on one line.
 */
export const SETUP_MATRIX = 'grid grid-cols-[minmax(0,1fr)_repeat(3,190px)_var(--control-h)] items-center gap-x-2.5';

/** The cards of the harness steps: as many across as fit at a readable width, folding away empty columns. */
export const HARNESS_GRID = 'grid grid-cols-[repeat(auto-fit,minmax(16rem,1fr))] gap-x-4 gap-y-4';

/**
 * The harness picker: the list at a fixed width beside the detail panel, stacked once the frame is narrower than 46rem (about 640px)
 * (the frame, not the window: a container query on the picker itself).
 */
export const PICKER = {
  frame: '@container',
  columns: 'grid grid-cols-1 gap-4 @min-[46rem]:grid-cols-[17rem_minmax(0,1fr)]',
  /** The least the detail panel is tall, so moving between harnesses does not move the buttons below it. */
  detailMinHeight: 'min-h-[20rem]',
} as const;
