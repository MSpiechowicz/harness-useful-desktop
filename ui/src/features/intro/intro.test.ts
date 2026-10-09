import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { HANDOFF_MS, IntroController, REDUCED_MS, TOTAL_MS } from './intro.svelte';

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('IntroController', () => {
  it('plays playing -> handoff -> done within the 1.6 s budget', () => {
    const intro = new IntroController();

    intro.play('full');
    expect(intro.phase).toBe('playing');
    expect(intro.active).toBe(true);

    vi.advanceTimersByTime(HANDOFF_MS);
    expect(intro.phase).toBe('handoff');

    vi.advanceTimersByTime(TOTAL_MS - HANDOFF_MS);
    expect(intro.phase).toBe('done');
    expect(intro.active).toBe(false);
    expect(TOTAL_MS).toBeLessThanOrEqual(1600);
  });

  it('skips instantly and cancels the remaining timeline', () => {
    const intro = new IntroController();
    intro.play('full');

    intro.skip();
    expect(intro.phase).toBe('done');

    vi.advanceTimersByTime(TOTAL_MS);
    expect(intro.phase).toBe('done');
  });

  it('only crossfades the shell under reduced motion', () => {
    const intro = new IntroController();

    intro.play('reduced');
    expect(intro.phase).toBe('handoff');

    vi.advanceTimersByTime(REDUCED_MS);
    expect(intro.phase).toBe('done');
  });

  it('does not play at all when motion is off', () => {
    const intro = new IntroController();

    intro.play('off');

    expect(intro.phase).toBe('done');
  });

  it('can be replayed after finishing', () => {
    const intro = new IntroController();
    intro.play('off');

    intro.play('full');

    expect(intro.phase).toBe('playing');
  });
});
