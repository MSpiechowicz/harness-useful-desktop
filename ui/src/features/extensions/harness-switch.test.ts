import { render, screen } from '@testing-library/svelte';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { ipc } from '$lib/ipc/client';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { samplePrefs } from '../../../tests/helpers';
import ExtensionsViewer from './ExtensionsViewer.svelte';

beforeAll(async () => {
  vi.stubEnv('VITE_TRANSPORT', 'mock');
  await harness.load();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('switching harness in the Extensions view', () => {
  it('shows a spinner instead of the old list and freezes the header until the new view is in', async () => {
    prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'omp' });
    render(ExtensionsViewer);
    await screen.findByRole('navigation', { name: 'Extensions' });
    const search = screen.getByRole('searchbox', { name: 'Search extensions and catalogs' });

    // PI's view never arrives, so the switch stays in flight.
    const invoke = ipc.invoke.bind(ipc);
    vi.spyOn(ipc, 'invoke').mockImplementation(((name: string, args: unknown) =>
      name === 'extensions_view' && (args as { harness: string }).harness === 'pi' ? new Promise(() => {}) : invoke(name as never, args as never)) as typeof ipc.invoke);
    prefs.value = samplePrefs({ motion: 'off', defaultHarness: 'pi' });

    expect(await screen.findByRole('status', { name: 'Loading extensions' })).toBeInTheDocument();
    expect(screen.queryByRole('navigation', { name: 'Extensions' })).toBeNull();
    // The header keeps its search field, but it takes no input while the spinner shows.
    expect(screen.getByRole('searchbox', { name: 'Search extensions and catalogs' })).toBe(search);
    expect(search.closest('[inert]')).not.toBeNull();
  });
});
