import { fireEvent, render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import Intro from './Intro.svelte';
import { intro } from './intro.svelte';

const logo = () => document.querySelector('harness-logo');
const finishLaunch = () => fireEvent(logo() as Element, new CustomEvent('finish', { bubbles: true, composed: true }));

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'full' });
});

afterEach(() => {
  intro.skip();
  vi.restoreAllMocks();
});

describe('Intro', () => {
  it('plays the short launch of the shared logo while the intro is active', async () => {
    render(Intro);

    intro.play('full');
    await nextTick();

    expect(logo()).toHaveAttribute('version', 'short');
    expect(logo()).toHaveAttribute('autoplay');
  });

  it.each([
    ['a key press', () => fireEvent.keyDown(window, { key: 'a' })],
    ['a click', () => fireEvent.pointerDown(window)],
  ])('skips straight to the finished state on %s', async (_name, input) => {
    render(Intro);
    intro.play('full');
    await nextTick();
    expect(logo()).not.toBeNull();

    await input();

    expect(intro.phase).toBe('done');
    expect(logo()).toBeNull();
  });

  it('hands over to the shell when the logo reports its launch finished', async () => {
    render(Intro);
    intro.play('full');
    await nextTick();

    await finishLaunch();
    await new Promise((resolve) => setTimeout(resolve, 350));

    expect(intro.phase).toBe('handoff');
  });

  it('lets the lockup leave before the next screen, then removes it', async () => {
    render(Intro);
    intro.play('full');
    await nextTick();
    expect(document.querySelector('[data-leaving]')).toBeNull();

    await finishLaunch();
    await new Promise((resolve) => setTimeout(resolve, 350));
    expect(document.querySelector('[data-leaving]')).not.toBeNull();
    expect(logo()).not.toBeNull();

    intro.skip();
    await nextTick();
    expect(logo()).toBeNull();
  });

  it('shows no logo under reduced motion, only a crossfade of the shell', async () => {
    prefs.value = samplePrefs({ motion: 'reduced' });
    render(Intro);

    intro.play('reduced');
    await nextTick();

    expect(logo()).toBeNull();
  });

  it.each([
    [true, [{ play: true }]],
    [false, []],
  ])('plays the native launch sound only when the intro sound is on (%s)', async (soundEnabled, expected) => {
    const invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(null as never);
    prefs.value = samplePrefs({ motion: 'full', soundEnabled });
    render(Intro);

    intro.play('full');
    await nextTick();

    expect(invoke.mock.calls.filter(([name]) => name === 'launch_sound').map(([, args]) => args)).toEqual(expected);
  });

  it('stops the sound when the intro is skipped', async () => {
    const invoke = vi.spyOn(ipc, 'invoke').mockResolvedValue(null as never);
    render(Intro);
    intro.play('full');
    await nextTick();

    await fireEvent.keyDown(window, { key: 'Escape' });

    expect(invoke).toHaveBeenLastCalledWith('launch_sound', { play: false });
  });
});

function nextTick(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}
