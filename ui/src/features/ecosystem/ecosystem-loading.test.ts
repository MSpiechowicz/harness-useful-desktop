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

/** Shows the view with the matching commands not answering until the returned function is called. */
function showHolding(held: (name: CommandName, args: { query?: { tool?: string } }) => boolean): () => void {
  const { spy } = freshBackend();
  const real = spy.getMockImplementation() as typeof ipc.invoke;
  let release: () => void = () => {};
  const gate = new Promise<void>((resolve) => (release = resolve));

  spy.mockImplementation(((name: CommandName, args: never) => (held(name, args ?? {}) ? gate.then(() => real(name as never, args)) : real(name as never, args))) as typeof ipc.invoke);
  render(EcosystemView);

  return release;
}

function spinners(): (string | null)[] {
  return [...document.querySelectorAll('svg[role="status"]')].map((spinner) => spinner.getAttribute('aria-label'));
}

describe('Ecosystem view while it loads', () => {
  const SPINNER = ['Looking for the Harness family'];

  function nothingShown(): void {
    expect(screen.queryByRole('heading', { level: 2, name: PANEL_TITLE })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { level: 2, name: 'Family' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { level: 3, name: 'Where the spend goes' })).not.toBeInTheDocument();
  }

  it('shows one spinner for the whole view while nothing has answered', async () => {
    showHolding((name) => name === 'ecosystem_status' || name === 'dashboard_availability' || name === 'dashboard_query');

    await waitFor(() => expect(spinners()).toEqual(SPINNER));
    nothingShown();
  });

  it('keeps that spinner when the dashboard answers before the family', async () => {
    const release = showHolding((name) => name === 'ecosystem_status');

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(spinners()).toEqual(SPINNER);
    nothingShown();

    release();
    await waitFor(() => expect(screen.getByRole('heading', { level: 3, name: 'Where the spend goes' })).toBeInTheDocument());
    expect(spinners()).toEqual([]);
  });

  it('keeps that spinner while a single card has not answered, then shows everything together', async () => {
    const release = showHolding((name, args) => name === 'dashboard_query' && args.query?.tool === 'limits');

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(spinners()).toEqual(SPINNER);
    nothingShown();

    release();

    await waitFor(() => expect(screen.getByRole('heading', { level: 3, name: 'Plan limits' })).toBeInTheDocument());
    expect(screen.getByRole('heading', { level: 3, name: 'Where the spend goes' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { level: 2, name: PANEL_TITLE })).toBeInTheDocument();
    expect(spinners()).toEqual([]);
  });
});
