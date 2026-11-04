/**
 * How long the spinner of a detail panel stays at least, in ms: a panel whose reads end at once would otherwise flash it. And how long it waits
 * at most: a read that hangs (a slow team database) must not hide the rest of the panel, whose sections each say they are still loading.
 * Tests run without either (they would wait for every panel they open), and test the gate itself with their own times.
 */
export const MIN_LOADING_MS = import.meta.env.VITEST ? 0 : 1000;
export const MAX_LOADING_MS = import.meta.env.VITEST ? 0 : 4000;

/**
 * Whether the panel of the item `key` (null while the panel is closed) may show its body: once everything it reads is in (`loaded`) and the spinner was on for `minMs`, or after
 * `maxMs` whatever is in. It is decided once each time the panel opens on an item: later reads of it (a refresh, a live update) never bring the spinner back, closing and opening the panel again does.
 * Call it while the component is created.
 */
export function useReveal(key: () => string | null, loaded: () => boolean, minMs = MIN_LOADING_MS, maxMs = MAX_LOADING_MS): { readonly shown: boolean } {
  let waited = $state<string | null>(null);
  let revealed = $state<string | null>(null);

  // Whether the panel was closed since the item was shown: opening it again reads everything again, so the spinner comes back.
  let left = false;

  $effect(() => {
    const current = key();

    if (current === null) {
      left = true;

      return;
    }

    if (left) {
      left = false;
      waited = null;
      revealed = null;
    }

    const least = setTimeout(() => (waited = current), minMs);
    const most = setTimeout(() => (revealed = current), Math.max(minMs, maxMs));

    return () => {
      clearTimeout(least);
      clearTimeout(most);
    };
  });

  $effect(() => {
    const current = key();

    if (current !== null && waited === current && loaded()) {
      revealed = current;
    }
  });

  // Without a least or a most time (the tests) there is no spinner at all: the body is there in the same tick as the item.
  const instant = minMs === 0 && maxMs === 0;

  return {
    get shown() {
      if (instant) {
        return key() !== null;
      }

      const current = key();

      // A closed panel keeps what it showed while it slides out.
      if (current === null) {
        return true;
      }

      return revealed === current;
    },
  };
}
