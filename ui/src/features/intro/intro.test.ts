import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { HANDOFF_DURATION_MS, HANDOFF_MS, IntroController, REDUCED_MS, TOTAL_MS } from './intro.svelte';

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('IntroController', () => {
  it('plays playing -> handoff -> done after the logo launch', () => {
    const intro = new IntroController();

    intro.play('full');
    expect(intro.phase).toBe('playing');
    expect(intro.active).toBe(true);

    vi.advanceTimersByTime(HANDOFF_MS);
    expect(intro.phase).toBe('handoff');

    vi.advanceTimersByTime(TOTAL_MS - HANDOFF_MS);
    expect(intro.phase).toBe('done');
    expect(intro.active).toBe(false);
    expect(TOTAL_MS).toBe(HANDOFF_MS + HANDOFF_DURATION_MS);
  });

  it('waits for the whole logo launch before the shell takes over', () => {
    const intro = new IntroController();
    intro.play('full');

    vi.advanceTimersByTime(HANDOFF_MS - 1);

    expect(intro.phase).toBe('playing');
    expect(HANDOFF_MS).toBeGreaterThanOrEqual(1900);
  });

  it('hands over as soon as the logo reports its launch finished', () => {
    const intro = new IntroController();
    intro.play('full');

    vi.advanceTimersByTime(2500);
    intro.launched();
    expect(intro.phase).toBe('handoff');

    vi.advanceTimersByTime(HANDOFF_DURATION_MS);
    expect(intro.phase).toBe('done');
  });

  it('ignores a late launch report once the intro was skipped', () => {
    const intro = new IntroController();
    intro.play('full');
    intro.skip();

    intro.launched();

    expect(intro.phase).toBe('done');
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
