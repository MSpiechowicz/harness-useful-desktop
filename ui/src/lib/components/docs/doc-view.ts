import { tick } from 'svelte';

/**
 * Closes the panel of a row and hands the focus back to the row it was opened from.
 * `clear` forgets which row is open, `focus` puts the focus on a row once the table is in place again.
 */
export async function closePanel(openId: string | null, clear: () => void, focus: (id: string) => void): Promise<void> {
  clear();

  if (openId !== null) {
    await tick();
    focus(openId);
  }
}

/** Escape in a filled search empties it, and only then closes the panel. */
export function emptySearchOnEscape(event: KeyboardEvent, query: string, clear: () => void): void {
  if (event.key === 'Escape' && query !== '') {
    event.preventDefault();
    event.stopPropagation();
    clear();
  }
}

/** Why a list that is read shows no row: the search found nothing (`search`), or the filters leave nothing (`filter`). Null while there are rows to show. */
export function emptyReason(loaded: boolean, shown: number, query: string, filtered: boolean): 'search' | 'filter' | null {
  if (!loaded || shown > 0) {
    return null;
  }

  return query.trim() === '' && filtered ? 'filter' : 'search';
}
