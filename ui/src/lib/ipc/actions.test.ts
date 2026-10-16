import { afterEach, describe, expect, it, vi } from 'vitest';

import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { openExternal } from './actions';
import { ipc } from './client';
import { IpcFailure } from './transport';

import { samplePrefs } from '../../../tests/helpers';

afterEach(() => {
  vi.restoreAllMocks();
  prefs.value = null;
  toasts.items.forEach((item) => toasts.dismiss(item.id));
});

describe('openExternal', () => {
  it('says where to allow a refused link, and takes the user there', async () => {
    vi.spyOn(ipc, 'invoke').mockRejectedValue(new IpcFailure({ code: 'forbidden', message: 'refused by the access rules' }));

    await openExternal('https://github.com/someone/repo/pull/12');

    const toast = toasts.items.at(-1);
    expect(toast?.description).toContain('Settings → Privacy & security');
    toast?.action?.run();
    expect(nav.current).toBe('settings');
    expect(viewState.settingsTarget).toBe('privacy');
  });

  it('does not point to the setting when it is already on (an http link is still refused)', async () => {
    prefs.value = samplePrefs({ openAnyLink: true });
    vi.spyOn(ipc, 'invoke').mockRejectedValue(new IpcFailure({ code: 'forbidden', message: 'refused by the access rules' }));

    await openExternal('http://example.com');

    expect(toasts.items.at(-1)).toMatchObject({ description: 'Only https links, or http links to this computer (localhost), without a user name or password open.', action: undefined });
  });

  it('reports other failures as they are', async () => {
    vi.spyOn(ipc, 'invoke').mockRejectedValue(new IpcFailure({ code: 'process', message: 'cannot open the browser' }));

    await openExternal('https://omp.sh');

    expect(toasts.items.at(-1)).toMatchObject({ title: 'Could not open the link', description: 'cannot open the browser', action: undefined });
  });
});
