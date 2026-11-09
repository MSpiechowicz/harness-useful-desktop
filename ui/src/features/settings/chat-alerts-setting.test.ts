import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import SettingsView from './SettingsView.svelte';

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off' });
  Element.prototype.scrollIntoView = vi.fn();
});

describe('Settings, Startup: chat alerts', () => {
  it('has both switches on by default, and the user can switch each off and save it', async () => {
    render(SettingsView);
    const alerts = screen.getByRole('switch', { name: 'Chat alerts' });
    const sound = screen.getByRole('switch', { name: 'Alert sound' });

    expect(alerts).toBeChecked();
    expect(sound).toBeChecked();
    expect(sound).toBeEnabled();

    await fireEvent.click(sound);

    await waitFor(() => expect(prefs.value?.alertSound).toBe(false));
    expect(prefs.value?.chatAlerts).toBe(true);

    await fireEvent.click(sound);
    await waitFor(() => expect(prefs.value?.alertSound).toBe(true));

    await fireEvent.click(alerts);

    await waitFor(() => expect(prefs.value?.chatAlerts).toBe(false));
    expect(alerts).not.toBeChecked();
  });

  it('disables the sound switch while chat alerts are off, says why, and keeps its choice', async () => {
    prefs.value = samplePrefs({ motion: 'off', chatAlerts: false, alertSound: true });
    render(SettingsView);
    const sound = screen.getByRole('switch', { name: 'Alert sound' });

    expect(sound).toBeDisabled();
    expect(sound).toBeChecked();
    expect(screen.getByText('Turn on chat alerts to hear the chime.', { selector: 'span:not([aria-hidden])' })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('switch', { name: 'Chat alerts' }));

    await waitFor(() => expect(sound).toBeEnabled());
    expect(screen.queryByText('Turn on chat alerts to hear the chime.', { selector: 'span:not([aria-hidden])' })).not.toBeInTheDocument();
  });
});
