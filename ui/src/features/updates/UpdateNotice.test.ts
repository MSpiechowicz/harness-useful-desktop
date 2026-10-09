import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type { UpdateInfo } from '$lib/ipc/bindings/UpdateInfo';
import { nav } from '$lib/stores/nav.svelte';
import { updates } from '$lib/stores/updates.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import UpdateNotice from './UpdateNotice.svelte';

const INFO: UpdateInfo = { version: '0.2.0', currentVersion: '0.1.0', date: null, notes: null };

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
});

afterEach(async () => {
  updates.state = { kind: 'idle' };
  nav.go('chat');
  window.history.replaceState(null, '', '/');
});

describe('UpdateNotice', () => {
  it('appears for an available update and goes away for this version on "Later"', async () => {
    updates.state = { kind: 'available', info: { ...INFO, version: '0.1.5' } };
    render(UpdateNotice);

    expect(screen.getByRole('region', { name: 'Update' })).toHaveTextContent('0.1.5');

    await fireEvent.click(screen.getByRole('button', { name: 'Later' }));

    await waitFor(() => expect(screen.queryByRole('region', { name: 'Update' })).toBeNull());
  });

  it('"View" opens Settings at the Updates section, which then shows the update itself', async () => {
    updates.state = { kind: 'available', info: { ...INFO, version: '0.4.0' } };
    render(UpdateNotice);

    await fireEvent.click(screen.getByRole('button', { name: 'View' }));

    expect(nav.current).toBe('settings');
    expect(viewState.settingsTarget).toBe('updates');
    await waitFor(() => expect(screen.queryByRole('region', { name: 'Update' })).toBeNull());
  });

  it('installs from the notice and ends with a restart prompt', async () => {
    await updates.check();
    render(UpdateNotice);

    await fireEvent.click(screen.getByRole('button', { name: 'Install' }));

    expect(await screen.findByRole('button', { name: 'Restart now' })).toBeInTheDocument();
    expect(updates.state.kind).toBe('readyToRelaunch');
  });

  it('shows progress while installing', () => {
    updates.state = { kind: 'downloading', info: { ...INFO, version: '0.5.0' }, progress: { phase: 'downloading', downloaded: 1, total: 4 } };
    render(UpdateNotice);

    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');
  });

  it('stays out of the way when a check found nothing or failed', () => {
    for (const state of [{ kind: 'upToDate', checkedAt: new Date() }, { kind: 'notConfigured' }, { kind: 'failed', during: 'check', reason: 'network', info: null }] as const) {
      updates.state = state;
      const { container, unmount } = render(UpdateNotice);

      expect(container.querySelector('section')).toBeNull();
      unmount();
    }
  });
});
