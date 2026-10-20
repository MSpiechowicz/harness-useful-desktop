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
 * The tiles of what a harness has on this machine, in one row across the panel (a container query on the section, so it
 * holds whether the picker is stacked or side by side). Each column is as wide as its tile's longest line needs and no
 * less than an equal share, so the tiles share the row equally until a long label or time needs more, and nothing is cut
 * off or wrapped in any language. Where even that does not fit, two columns take the place of the row: the panel is no
 * longer fixed in height there, which is why the picker stacks (`PICKER`) well before the panel is that narrow.
 * A tile is the number on top and its label under it, both from the top, so numbers and labels of the tiles in a row
 * start at the same height.
 */
export const FACTS = {
  section: '@container',
  row: 'grid grid-flow-col auto-cols-[minmax(max-content,1fr)] gap-2 @max-[33.5rem]:grid-flow-row @max-[33.5rem]:grid-cols-[repeat(auto-fit,minmax(11.5rem,1fr))]',
  tile: 'flex flex-col gap-1 rounded-md border border-line bg-surface-1 px-4 py-3',
  /** The line of the number: one height for the placeholder and the number, so nothing moves when it arrives. */
  value: 'flex h-6 items-center',
  label: 'text-xs leading-4 text-fg-3',
} as const;

/**
 * The harness picker: the list at a fixed width beside the detail panel, stacked once the frame is narrower than 55rem (about 770px)
 * (the frame, not the window: a container query on the picker itself). That is the narrowest frame whose panel still holds
 * the four tiles of `FACTS` in one row.
 */
export const PICKER = {
  frame: '@container',
  columns: 'grid grid-cols-1 gap-4 @min-[55rem]:grid-cols-[19.5rem_minmax(0,1fr)]',
  /**
   * The least the detail panel is tall, so moving between harnesses does not move what is below it. Measured in WebKitGTK:
   * the tallest realistic panel (a missing harness with a wrapping install command, an error line and two buttons) is
   * 316px at the narrowest side by side. An installed harness fills the panel with its facts and actions: 341px in
   * WebKitGTK, so 24.5rem (343px) holds it with about 2px to spare.
   */
  detailMinHeight: 'min-h-[25.5rem]',
} as const;
