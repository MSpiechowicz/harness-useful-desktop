/** The look of a chip in the composer footer (project, branch, changes): as tall as a button, quiet until hovered. */
export const CHIP = 'inline-flex h-control min-w-0 items-center gap-1.5 rounded-md px-2 text-sm text-fg-3 transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg';

/** A chip that opens a menu keeps its hover look while the menu is open. */
export const CHIP_OPEN = 'aria-expanded:bg-hover aria-expanded:text-fg';
