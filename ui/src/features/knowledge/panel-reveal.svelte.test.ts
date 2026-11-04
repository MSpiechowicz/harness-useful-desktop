import { flushSync } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useReveal } from './panel-reveal.svelte';

describe('the spinner of a detail panel', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  /** A gate for `key` whose reads are in once `loaded` is set, inside an effect root as a component would hold it. */
  function gate(min: number, max: number) {
    const state = $state({ key: 'a' as string | null, loaded: false });
    let reveal: { readonly shown: boolean } = { shown: false };
    const stop = $effect.root(() => {
      reveal = useReveal(() => state.key, () => state.loaded, min, max);
    });
    flushSync();

    return { state, reveal: () => reveal.shown, stop };
  }

  it('stays for the least time even when everything is in at once, then shows the body', () => {
    const { state, reveal, stop } = gate(1000, 4000);
    state.loaded = true;
    flushSync();

    expect(reveal()).toBe(false);

    vi.advanceTimersByTime(1000);
    flushSync();

    expect(reveal()).toBe(true);
    stop();
  });

  it('waits for the reads after the least time, and shows whatever is in after the most', () => {
    const { reveal, stop } = gate(1000, 4000);

    vi.advanceTimersByTime(1000);
    flushSync();
    expect(reveal()).toBe(false);

    vi.advanceTimersByTime(3000);
    flushSync();
    expect(reveal()).toBe(true);
    stop();
  });

  it('never comes back for the same item, and starts again for the next one', () => {
    const { state, reveal, stop } = gate(1000, 4000);
    state.loaded = true;
    vi.advanceTimersByTime(1000);
    flushSync();
    expect(reveal()).toBe(true);

    // A refresh of the same item reads again: the body stays.
    state.loaded = false;
    flushSync();
    expect(reveal()).toBe(true);

    state.key = 'b';
    state.loaded = true;
    flushSync();
    expect(reveal()).toBe(false);

    vi.advanceTimersByTime(1000);
    flushSync();
    expect(reveal()).toBe(true);
    stop();
  });

  it('comes back when the panel is closed and opened again on the same item', () => {
    const { state, reveal, stop } = gate(1000, 4000);
    state.loaded = true;
    vi.advanceTimersByTime(1000);
    flushSync();
    expect(reveal()).toBe(true);

    state.key = null;
    flushSync();
    // While it slides out it keeps its body.
    expect(reveal()).toBe(true);

    state.key = 'a';
    flushSync();
    expect(reveal()).toBe(false);

    vi.advanceTimersByTime(1000);
    flushSync();
    expect(reveal()).toBe(true);
    stop();
  });
});
