// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type { UpdateInfo } from '$lib/ipc/bindings/UpdateInfo';
import { app } from '$lib/stores/app.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { updates, type UpdateState } from '$lib/stores/updates.svelte';

import { samplePrefs } from '../../../tests/helpers';
import UpdatesSection from './UpdatesSection.svelte';

const INFO: UpdateInfo = { version: '0.2.0', currentVersion: '0.1.0', date: '2026-10-08T18:42:11Z', notes: '## 0.2.0\n\n* faster start' };

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  await app.load();
});

afterEach(() => {
  updates.state = { kind: 'idle' };
  prefs.value = samplePrefs({ motion: 'off' });
  window.history.replaceState(null, '', '/');
});

function show(state: UpdateState): void {
  updates.state = state;
  render(UpdatesSection, { id: 'updates' });
}

describe('Updates settings', () => {
  it('checks on demand and then offers the update with its notes', async () => {
    render(UpdatesSection, { id: 'updates' });

    await fireEvent.click(screen.getByRole('button', { name: 'Check now' }));

    expect(await screen.findByRole('button', { name: 'Install & restart' })).toBeInTheDocument();
    expect(within(screen.getByRole('region', { name: 'Release notes' })).getByText(/keep the selected file/)).toBeInTheDocument();
  });

  it('says so when the app is up to date', async () => {
    window.history.replaceState(null, '', '/?update=current');
    render(UpdatesSection, { id: 'updates' });

    await fireEvent.click(screen.getByRole('button', { name: 'Check now' }));

    await waitFor(() => expect(updates.state.kind).toBe('upToDate'));
    expect(screen.queryByRole('button', { name: 'Install & restart' })).toBeNull();
  });

  it('renders untrusted release notes as text, never as markup', () => {
    show({ kind: 'available', info: { ...INFO, notes: 'Hi <img src=x onerror="alert(1)"> <script>alert(2)</script> <b>bold</b>' } });

    const notes = screen.getByRole('region', { name: 'Release notes' });

    expect(notes.querySelector('img, script, b')).toBeNull();
    expect(notes.textContent).toContain('<script>alert(2)</script>');
  });

  it('keeps the policy the user picks', async () => {
    render(UpdatesSection, { id: 'updates' });

    await fireEvent.click(screen.getByRole('radio', { name: 'Off' }));

    await waitFor(() => expect(prefs.value?.updatePolicy).toBe('off'));
    expect(screen.getByRole('radio', { name: 'Off' })).toBeChecked();
  });

  it('updates extensions by default and lets the user turn that off', async () => {
    render(UpdatesSection, { id: 'updates' });
    const toggle = screen.getByRole('switch', { name: 'Update extensions automatically' });

    expect(toggle).toBeChecked();

    await fireEvent.click(toggle);

    await waitFor(() => expect(prefs.value?.autoUpdateExtensions).toBe(false));
  });

  it('refreshes catalogs by default, as a switch of its own that the user can turn off', async () => {
    render(UpdatesSection, { id: 'updates' });
    const toggle = screen.getByRole('switch', { name: 'Refresh catalogs automatically' });

    expect(toggle).toBeChecked();
    expect(screen.getByRole('switch', { name: 'Update extensions automatically' })).toBeChecked();

    await fireEvent.click(toggle);

    await waitFor(() => expect(prefs.value?.autoRefreshCatalogs).toBe(false));
    expect(prefs.value?.autoUpdateExtensions).toBe(true);
    expect(screen.getByRole('switch', { name: 'Update extensions automatically' })).toBeChecked();
  });

  it('shows both automatic switches off and unavailable until the preferences have arrived', () => {
    prefs.value = null;
    render(UpdatesSection, { id: 'updates' });

    for (const name of ['Update extensions automatically', 'Refresh catalogs automatically']) {
      const toggle = screen.getByRole('switch', { name });

      expect(toggle).not.toBeChecked();
      expect(toggle).toBeDisabled();
    }
  });

  it('promises the catalog schedule the app keeps: about thirty seconds after start, then every thirty minutes', () => {
    render(UpdatesSection, { id: 'updates' });

    expect(screen.getByText(/about thirty seconds after it starts, then every thirty minutes/)).toBeInTheDocument();
  });

  it('says the extensions switch also covers the installed Harness family members', () => {
    render(UpdatesSection, { id: 'updates' });

    expect(screen.getByText(/Harness Memory and Harness Dashboard/)).toBeInTheDocument();
  });

  it('explains a build without updates as a neutral note, not an error', () => {
    show({ kind: 'notConfigured' });

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.getByRole('button', { name: 'Open releases' })).toBeInTheDocument();
  });

  it('does not offer a check where it can only fail again or repeat the update', () => {
    show({ kind: 'notConfigured' });
    expect(screen.queryByRole('button', { name: 'Check now' })).toBeNull();
    expect(screen.getByRole('button', { name: 'Open releases' })).toBeInTheDocument();
  });

  it('does not offer a second check once the update is installed', () => {
    show({ kind: 'readyToRelaunch', info: INFO });

    expect(screen.queryByRole('button', { name: 'Check now' })).toBeNull();
  });

  it('leaves out a notes heading that only repeats the version', () => {
    show({ kind: 'available', info: INFO });

    const notes = screen.getByRole('region', { name: 'Release notes' });

    expect(within(notes).queryByRole('heading')).toBeNull();
    expect(notes).toHaveTextContent('faster start');
  });

  it('keeps notes headings that say something else', () => {
    show({ kind: 'available', info: { ...INFO, notes: '## Highlights\n\n* faster start' } });

    expect(within(screen.getByRole('region', { name: 'Release notes' })).getByRole('heading', { name: 'Highlights' })).toBeInTheDocument();
  });

  it('shows a signature failure as a generic security message without a retry', () => {
    show({ kind: 'failed', during: 'install', reason: 'verification', info: INFO });

    expect(screen.getByRole('alert')).toHaveTextContent(/could not be verified and was not installed/i);
    expect(screen.queryByRole('button', { name: 'Try again' })).toBeNull();
  });

  it('offers a retry after a network failure during install', () => {
    show({ kind: 'failed', during: 'install', reason: 'network', info: INFO });

    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('shows download progress with its size', () => {
    show({ kind: 'downloading', info: INFO, progress: { phase: 'downloading', downloaded: 1_048_576, total: 2_097_152 } });
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '50');
    expect(screen.getByText(/1\.0 MB of 2\.0 MB/)).toBeInTheDocument();
  });

  it('shows an indeterminate bar when the total is unknown', () => {
    show({ kind: 'downloading', info: INFO, progress: { phase: 'downloading', downloaded: 4096, total: null } });

    expect(screen.getByRole('progressbar')).not.toHaveAttribute('aria-valuenow');
  });

  it('finishes with a restart button, and on Windows says the installer does it', async () => {
    show({ kind: 'readyToRelaunch', info: INFO });
    expect(screen.getByRole('button', { name: 'Restart to finish' })).toBeInTheDocument();

    app.info = { ...app.info!, os: 'windows' };
    await waitFor(() => expect(screen.getByRole('button', { name: 'Finish update' })).toBeInTheDocument());
    expect(screen.getByText(/installer will close Harness Desktop/i)).toBeInTheDocument();

    app.info = { ...app.info!, os: 'linux' };
  });
});
