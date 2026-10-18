import { render, screen } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { harness } from '$lib/stores/harness.svelte';
import { CONFIG_POLL_MS, RETRY_BACKOFF_MS } from '$lib/stores/live';
import { prefs } from '$lib/stores/prefs.svelte';
import { configView } from '$lib/stores/views.svelte';

import { samplePrefs } from '../../../tests/helpers';
import ConfigViewer from './ConfigViewer.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
  await harness.load();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  configView.sync(null, null);
});

/** The backend as it is, except that configuration reads fail while `state.failing` is set. */
function flakyConfig(state: { failing: boolean }): void {
  const real = ipc.invoke.bind(ipc);

  vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) =>
    name === 'config_view' && state.failing ? Promise.reject({ code: 'io', message: 'unreadable' }) : real(name as never, args as never)) as typeof ipc.invoke);
}

describe('Config recovers by itself', () => {
  it('tries a configuration that could not be read again by itself, with a longer wait each time', async () => {
    const state = { failing: true };
    flakyConfig(state);
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    render(ConfigViewer);
    await vi.advanceTimersByTimeAsync(100);
    expect(screen.getByText('Could not read the configuration')).toBeInTheDocument();

    // The first retry fails too: the message stays where it is, with no flash of the loading spinner.
    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[0]);
    expect(screen.getByText('Could not read the configuration')).toBeInTheDocument();
    expect(screen.queryByRole('status', { name: /loading/i })).toBeNull();

    state.failing = false;
    await vi.advanceTimersByTimeAsync(RETRY_BACKOFF_MS[1] + 100);

    expect(screen.queryByText('Could not read the configuration')).toBeNull();
    expect(screen.getByRole('navigation', { name: 'Configuration files' })).toBeInTheDocument();
  });

  it('waits for a slow read to end instead of asking again on every tick', async () => {
    const real = ipc.invoke.bind(ipc);
    let reads = 0;
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) => {
      if (name !== 'config_view') {
        return real(name as never, args as never);
      }

      reads += 1;

      return reads === 2 ? gate.then(() => real(name as never, args as never)) : real(name as never, args as never);
    }) as typeof ipc.invoke);
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
    render(ConfigViewer);
    await vi.waitFor(() => expect(screen.getByRole('navigation', { name: 'Configuration files' })).toBeInTheDocument());
    expect(reads).toBe(1);

    await vi.advanceTimersByTimeAsync(CONFIG_POLL_MS * 4);
    expect(reads).toBe(2);

    release();
    // The mock backend answers on a real timer of its own.
    await new Promise((resolve) => setTimeout(resolve, 60));
    await vi.advanceTimersByTimeAsync(CONFIG_POLL_MS);
    expect(reads).toBe(3);
  });
});
