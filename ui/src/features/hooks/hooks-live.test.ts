import { fireEvent, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { viewState } from '$lib/stores/view-state.svelte';

import { hooks } from './hooks-data.svelte';
import { fakeHooks, registerHooksHooks, rowOf, showHooks } from './hooks-test-support';

registerHooksHooks();

const LINT = 'Run lint after edits';

describe('an open hook', () => {
  it('shows what an agent changed in it as soon as the backend says the hooks folder changed', async () => {
    const { state } = fakeHooks();
    await showHooks();
    const stop = hooks.start();
    await fireEvent.click(rowOf(LINT));
    const panel = await screen.findByRole('region', { name: LINT });
    const id = viewState.hookId ?? '';

    state.agentChanges(id);

    expect(await screen.findByRole('region', { name: `${LINT} v2` })).toBe(panel);
    expect(viewState.hookId).toBe(id);
    stop();
  });

  it('shows what an agent changed in it at the next poll, when no event came', async () => {
    const { state } = fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf(LINT));
    await screen.findByRole('region', { name: LINT });

    state.agentChanges(viewState.hookId ?? '');
    window.dispatchEvent(new Event('focus'));

    expect(await screen.findByRole('region', { name: `${LINT} v2` })).toBeInTheDocument();
  });

  it('closes its panel when an agent deleted the hook', async () => {
    const { state } = fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf(LINT));
    await screen.findByRole('region', { name: LINT });
    const open = hooks.find(viewState.hookId);

    state.delete(open?.id ?? '', open?.sha256 ?? '');
    window.dispatchEvent(new Event('focus'));

    await waitFor(() => expect(viewState.hookId).toBeNull());
    await waitFor(() => expect(screen.queryByRole('region', { name: LINT })).not.toBeInTheDocument());
  });
});
