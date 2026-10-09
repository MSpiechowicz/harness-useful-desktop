import { describe, expect, it } from 'vitest';

import { fakeMedia } from '../../../tests/helpers';
import { MediaQuery } from '$lib/design/theme.svelte';
import type { ThemePref } from '$lib/ipc/bindings/ThemePref';

import { ThemeStore } from './theme.svelte';

function setup(preference: ThemePref, systemDark: boolean) {
  const media = fakeMedia(systemDark);
  const root = document.createElement('html');
  const store = new ThemeStore(() => preference, new MediaQuery('(prefers-color-scheme: dark)', media.source), root);

  return { store, media, root };
}

describe('ThemeStore', () => {
  it('uses an explicit preference regardless of the OS', () => {
    expect(setup('light', true).store.resolved).toBe('light');
    expect(setup('dark', false).store.resolved).toBe('dark');
  });

  it('follows the OS when the preference is system, including live changes', () => {
    const { store, media } = setup('system', false);
    expect(store.resolved).toBe('light');

    media.set(true);

    expect(store.resolved).toBe('dark');
  });

  it('writes the resolved theme to the root element and tracks changes', async () => {
    const { store, media, root } = setup('system', true);
    const stop = store.start();
    await Promise.resolve();
    expect(root.dataset.theme).toBe('dark');

    media.set(false);
    await Promise.resolve();
    expect(root.dataset.theme).toBe('light');
    stop();
  });
});
