// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { app } from '$lib/stores/app.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import HarnessSettings from './HarnessSettings.svelte';

const SWITCH = 'Check harness CLIs for updates';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  await app.load();
});

afterEach(() => {
  prefs.value = samplePrefs({ motion: 'off' });
});

describe('Harness settings', () => {
  it('checks the harness CLIs by default, as a switch of its own that the user can turn off', async () => {
    render(HarnessSettings);
    const toggle = screen.getByRole('switch', { name: SWITCH });

    expect(toggle).toBeChecked();

    await fireEvent.click(toggle);

    await waitFor(() => expect(prefs.value?.checkHarnessUpdates).toBe(false));
    expect(prefs.value?.checkCompanionUpdates).toBe(true);
    await expect(ipc.invoke('harness_releases', { installed: [{ harness: 'omp', version: '0.0.1' }] })).resolves.toEqual([]);
  });

  it('shows the switch off and unavailable until the preferences have arrived', () => {
    prefs.value = null;
    render(HarnessSettings);
    const toggle = screen.getByRole('switch', { name: SWITCH });

    expect(toggle).not.toBeChecked();
    expect(toggle).toBeDisabled();
  });
});
