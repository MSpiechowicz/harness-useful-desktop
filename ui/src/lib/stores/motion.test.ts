import { describe, expect, it } from 'vitest';

import { fakeMedia } from '../../../tests/helpers';
import { MediaQuery } from '$lib/design/theme.svelte';
import type { MotionPref } from '$lib/ipc/bindings/MotionPref';

import { effectiveMotion, MotionStore } from './motion.svelte';

describe('effectiveMotion', () => {
  it.each([
    ['full', false, 'full'],
    ['full', true, 'reduced'],
    ['reduced', false, 'reduced'],
    ['reduced', true, 'reduced'],
    ['off', false, 'off'],
    ['off', true, 'off'],
  ] as const)('pref %s with OS reduce=%s gives %s', (pref, osReduces, expected) => {
    expect(effectiveMotion(pref, osReduces)).toBe(expected);
  });
});

describe('MotionStore', () => {
  function setup(preference: MotionPref, osReduces: boolean) {
    const media = fakeMedia(osReduces);
    const root = document.createElement('html');
    const store = new MotionStore(() => preference, new MediaQuery('(prefers-reduced-motion: reduce)', media.source), root);

    return { store, media, root };
  }

  it('is the more restrictive of the setting and the OS, reacting to OS changes', () => {
    const { store, media } = setup('full', false);
    expect(store.level).toBe('full');

    media.set(true);

    expect(store.level).toBe('reduced');
  });

  it('mirrors the level onto data-motion', async () => {
    const { store, root } = setup('off', false);
    const stop = store.start();
    await Promise.resolve();

    expect(root.dataset.motion).toBe('off');
    stop();
  });
});
