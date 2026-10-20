import { tick, untrack } from 'svelte';

/** Starts a table from its first page again whenever `key` (what narrows or reorders it) changes. A page kept from before, as the view was left and opened again, stays. Call it while the view is set up. */
export function startPageOver(key: () => string, reset: () => void): void {
  let seen: string | null = null;

  $effect(() => {
    const by = key();

    untrack(() => {
      if (seen !== null && seen !== by) {
        reset();
      }

      seen = by;
    });
  });
}

interface Reveal {
  /** The row whose panel is open. */
  openId: () => string | null;
  /** Whether the row is in the list at all, the filters aside. */
  listed: (id: string) => boolean;
  /** The place of the row among those the filters keep, in the order of the table; null when they hide it. */
  indexOf: (id: string) => number | null;
  pageSize: number;
  setPage: (page: number) => void;
  /** Lets go of everything that narrows the list: the Scope, the search and the other filters. */
  clearFilters: () => void;
}

async function reveal(id: string, { indexOf, pageSize, setPage, clearFilters }: Reveal): Promise<void> {
  let index = indexOf(id);

  if (index === null) {
    clearFilters();
    // The page starts over with the filters (see `startPageOver`): the jump to the row goes after it.
    await tick();
    index = indexOf(id);
  }

  if (index !== null) {
    setPage(Math.floor(index / pageSize));
  }
}

/**
 * Keeps the row whose panel opens in sight: a row the filters hide shows once they are cleared, and the table turns to the page it is on.
 * It acts once for each row opened (a saved row, one chosen from the run history), not as the list changes under an open panel. Call it while the view is set up.
 */
export function revealOpenRow(input: Reveal): void {
  let handled: string | null = null;

  $effect(() => {
    const id = input.openId();

    if (id === null) {
      handled = null;

      return;
    }

    if (id !== handled && input.listed(id)) {
      handled = id;
      untrack(() => void reveal(id, input));
    }
  });
}
