import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import { LEAVING_ATTRIBUTE, READY_ATTRIBUTE, readyAfterMount, titleOut } from './header-motion';
import { DURATION } from './motion';

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'full' });
});
afterEach(() => {
  vi.useRealTimers();
  prefs.value = samplePrefs({ motion: 'off' });
});

/** A slot with the outgoing title in it, and the way to play its transition to a given moment. */
function slotWithOutgoing() {
  const slot = document.createElement('div');
  const outgoing = document.createElement('div');
  slot.append(outgoing);

  const transition = titleOut(outgoing);
  const total = transition.duration as number;
  const tick = transition.tick as (remaining: number, elapsed: number) => void;
  const at = (ms: number): number => {
    tick(1 - ms / total, ms / total);

    return +outgoing.style.opacity;
  };

  return { slot, outgoing, at };
}

describe('the title that leaves the top bar', () => {
  it('stays at full strength while no title has taken its place, however late that is', () => {
    const { outgoing, at } = slotWithOutgoing();

    expect(outgoing).toHaveAttribute(LEAVING_ATTRIBUTE);
    expect(at(0)).toBe(1);
    expect(at(DURATION.base)).toBe(1);
    expect(at(DURATION.slow - 1)).toBe(1);
  });

  it('fades over the time the incoming title fades in once it exists, so the two always sum to one', () => {
    const { slot, at } = slotWithOutgoing();
    expect(at(DURATION.slow - 100)).toBe(1);

    const incoming = document.createElement('div');
    slot.append(incoming);

    expect(at(DURATION.slow - 100)).toBe(1);
    expect(at(DURATION.slow - 100 + DURATION.base / 2)).toBeCloseTo(0.5, 5);
    expect(at(DURATION.slow - 100 + DURATION.base)).toBe(0);
  });

  it('fades straight away when the incoming title is already there', () => {
    const { slot, at } = slotWithOutgoing();
    slot.append(document.createElement('div'));

    expect(at(0)).toBe(1);
    expect(at(DURATION.base / 2)).toBeCloseTo(0.5, 5);
  });

  it('does not treat another leaving title as the one that takes its place', () => {
    const { slot, at } = slotWithOutgoing();
    const alsoLeaving = document.createElement('div');
    alsoLeaving.setAttribute(LEAVING_ATTRIBUTE, '');
    slot.append(alsoLeaving);

    expect(at(DURATION.base)).toBe(1);
  });

  it('gives up waiting after a while and fades anyway, so a view without a title does not leave the old one behind', () => {
    const { at } = slotWithOutgoing();

    expect(at(DURATION.slow)).toBe(1);
    expect(at(DURATION.slow + DURATION.base / 2)).toBeCloseTo(0.5, 5);
    expect(at(DURATION.slow + DURATION.base)).toBe(0);
  });
});

describe('the title that leaves the top bar with motion off', () => {
  it('is hidden outright instead of getting a not-a-number opacity', () => {
    prefs.value = samplePrefs({ motion: 'off' });
    const outgoing = document.createElement('div');
    const transition = titleOut(outgoing);

    (transition.tick as (remaining: number, elapsed: number) => void)(0, 1);
    expect(Number.isNaN(+outgoing.style.opacity)).toBe(false);
    expect(outgoing.style.opacity).toBe('0');
    expect(outgoing.style.visibility).toBe('hidden');

    (transition.tick as (remaining: number, elapsed: number) => void)(1, 0);
    expect(outgoing.style.opacity).toBe('0');
  });
});

describe('a fixed part that is ready', () => {
  it('is marked a moment after it appears, so a half-built one is not swapped in for the one on screen', () => {
    vi.useFakeTimers();
    const node = document.createElement('div');

    readyAfterMount(node);
    expect(node).not.toHaveAttribute(READY_ATTRIBUTE);

    vi.advanceTimersByTime(DURATION.fast - 1);
    expect(node).not.toHaveAttribute(READY_ATTRIBUTE);

    vi.advanceTimersByTime(1);
    expect(node).toHaveAttribute(READY_ATTRIBUTE);
  });

  it('is never marked once it is gone', () => {
    vi.useFakeTimers();
    const node = document.createElement('div');

    readyAfterMount(node).destroy();
    vi.advanceTimersByTime(DURATION.slow);

    expect(node).not.toHaveAttribute(READY_ATTRIBUTE);
  });

  it('is ready at once when motion is off', () => {
    prefs.value = samplePrefs({ motion: 'off' });
    const node = document.createElement('div');

    readyAfterMount(node);

    expect(node).toHaveAttribute(READY_ATTRIBUTE);
  });
});
