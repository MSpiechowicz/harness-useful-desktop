import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';

import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import { app } from '$lib/stores/app.svelte';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { project } from '$lib/stores/project.svelte';

import { diagnosticsText } from './diagnostics';
import PrivacySection from './PrivacySection.svelte';

// The component talks to the real stores, which talk to the in-memory mock backend.
beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');

  await prefs.load();
  await Promise.all([app.load(), harness.load(), project.load()]);
});

describe('PrivacySection', () => {
  it('states what is stored, never read, redacted, sent and logged', () => {
    render(PrivacySection);

    for (const title of ['Stored on this device', 'Never read', 'Shown redacted', 'Network', 'No telemetry', 'Agents', 'Git', 'Logs']) {
      expect(screen.getByText(title)).toBeInTheDocument();
    }

    expect(screen.getByText(/No chat content, tokens or passwords are saved/)).toBeInTheDocument();
    expect(screen.getByText(/mcp-auth\.json/)).toBeInTheDocument();
    expect(screen.getByText(/never what you say/)).toBeInTheDocument();
  });

  // The statements below are promises (docs/security-privacy.md#network); each topic must stay in the fact that owns it.
  it('names every network use in the Network fact, and every other use as one that needs you', () => {
    render(PrivacySection);
    const network = screen.getByText('Network').closest('li')!;

    for (const topic of [/latest\.json/, /update key/, /update setting/, /turned off/, /only when you click/, /stdio/, /no network/, /may download packages/, /only after you confirm/, /as in your terminal/]) {
      expect(network).toHaveTextContent(topic);
    }

    expect(screen.queryByText(/no network requests of its own/)).toBeNull();
  });

  it('promises not to edit harness settings, since a per-session approval choice exists', () => {
    render(PrivacySection);
    const agents = screen.getByText('Agents').closest('li')!;

    expect(agents).toHaveTextContent(/never edits your harness/);
    expect(agents).not.toHaveTextContent(/never changes your approval mode/);
  });

  it('forgets every recent project and tells the user', async () => {
    expect(project.recents.length).toBeGreaterThan(1);
    render(PrivacySection);

    await fireEvent.click(screen.getByRole('button', { name: 'Clear recent projects' }));
    expect(project.recents.length).toBeGreaterThan(1);
    await fireEvent.click(await screen.findByRole('button', { name: 'Clear list' }));

    await waitFor(() => expect(project.recents).toEqual([]));
    expect(screen.getByRole('button', { name: 'Clear recent projects' })).toBeDisabled();
  });

  it('copies diagnostics without paths', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    render(PrivacySection);

    await fireEvent.click(screen.getByRole('button', { name: 'Copy diagnostics' }));

    await waitFor(() => expect(writeText).toHaveBeenCalledOnce());
    const copied: string = writeText.mock.calls[0][0];
    expect(copied).toContain(app.info!.version);
    expect(copied).not.toMatch(/\/home\/|\/Users\/|[A-Z]:\\/);
  });
});

describe('diagnosticsText', () => {
  it('lists version, platform and each harness status', () => {
    const missing = { info: { name: 'Pi' }, detection: { installed: false } } as HarnessSummary;
    const present = { info: { name: 'Oh My Pi' }, detection: { installed: true, version: '18.6.1', versionSupport: 'supported', binaryPath: '/home/me/bin/omp' } } as HarnessSummary;

    const text = diagnosticsText({ name: 'Harness Desktop', version: '0.1.0', os: 'linux', arch: 'x86_64', debug: false }, [present, missing]);

    expect(text).toBe('Harness Desktop 0.1.0 (linux x86_64)\nOh My Pi: installed, 18.6.1 (supported)\nPi: not installed');
  });
});
