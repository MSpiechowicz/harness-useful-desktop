import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import Intro from './Intro.svelte';
import { intro } from './intro.svelte';

const logo = () => screen.queryByRole('img', { name: 'Harness Desktop', hidden: true });

beforeEach(() => {
  prefs.value = samplePrefs({ motion: 'full' });
});

afterEach(() => intro.skip());

describe('Intro', () => {
  it('plays the logo launch on the dark canvas while the intro is active', async () => {
    render(Intro);

    intro.play('full');
    await nextTick();

    expect(logo()).toHaveAttribute('data-launching');
    expect(logo()?.closest('.theme-dark')).not.toBeNull();
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

    await fireEvent(document.querySelector('[data-part="cursor"]') as Element, new Event('animationend', { bubbles: true }));

    expect(intro.phase).toBe('handoff');
  });

  it('shows no logo under reduced motion, only a crossfade of the shell', async () => {
    prefs.value = samplePrefs({ motion: 'reduced' });
    render(Intro);

    intro.play('reduced');
    await nextTick();

    expect(logo()).toBeNull();
  });
});

function nextTick(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}
