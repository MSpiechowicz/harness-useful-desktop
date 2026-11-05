import { render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type { CommandName } from '$lib/ipc/commands';
import { ipc } from '$lib/ipc/client';
import { prefs } from '$lib/stores/prefs.svelte';

import { freshBackend } from '../../../tests/ecosystem-backend';
import { samplePrefs } from '../../../tests/helpers';
import EcosystemView from './EcosystemView.svelte';

const PANEL_TITLE = 'Your usage';

beforeAll(() => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off' });
});

afterEach(() => {
  vi.restoreAllMocks();
});

/** Shows the view with these commands not answering until the returned function is called. */
function showHolding(held: CommandName[]): () => void {
  const { spy } = freshBackend();
  const real = spy.getMockImplementation() as typeof ipc.invoke;
  let release: () => void = () => {};
  const gate = new Promise<void>((resolve) => (release = resolve));

  spy.mockImplementation(((name: CommandName, args: never) => (held.includes(name) ? gate.then(() => real(name as never, args)) : real(name as never, args))) as typeof ipc.invoke);
  render(EcosystemView);

  return release;
}

function spinners(): (string | null)[] {
  return [...document.querySelectorAll('svg[role="status"]')].map((spinner) => spinner.getAttribute('aria-label'));
}

describe('Ecosystem view while it loads', () => {
  it('shows one spinner, in the diagram, until the family is read', async () => {
    showHolding(['ecosystem_status', 'dashboard_availability', 'dashboard_query']);

    await waitFor(() => expect(spinners()).toEqual(['Looking for the Harness family']));
  });

  it('keeps that one spinner when the dashboard answers before the family', async () => {
    showHolding(['ecosystem_status', 'dashboard_query']);

    await waitFor(() => expect(screen.getByRole('heading', { level: 2, name: PANEL_TITLE })).toBeInTheDocument());
    expect(spinners()).toEqual(['Looking for the Harness family']);
  });

  it('moves to the dashboard check once the family is read', async () => {
    showHolding(['dashboard_availability', 'dashboard_query']);

    await waitFor(() => expect(spinners()).toEqual(['Checking the dashboard']));
  });

  it('shows one spinner for the panel, not one per card, until its first answer', async () => {
    const release = showHolding(['dashboard_query']);

    await waitFor(() => expect(spinners()).toEqual(['Loading usage']));
    expect(screen.queryByRole('heading', { level: 3, name: 'Where the spend goes' })).not.toBeInTheDocument();

    release();

    await waitFor(() => expect(screen.getByRole('heading', { level: 3, name: 'Where the spend goes' })).toBeInTheDocument());
    await waitFor(() => expect(spinners()).toEqual([]));
  });
});
