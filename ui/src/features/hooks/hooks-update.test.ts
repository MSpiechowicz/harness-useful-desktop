import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { hooks } from './hooks-data.svelte';
import { calls, fakeHooks, registerHooksHooks, rowOf, showHooks } from './hooks-test-support';

registerHooksHooks();

const SAVE = 'Save what you learned';
const NOTICE = 'A newer version of this Harness family hook is available.';

describe('the family catalog in the hooks store', () => {
  it('stays unknown until the catalog is read, so nothing says an update waits', () => {
    expect(hooks.family).toBeNull();
    expect(hooks.updates).toBe(0);
    expect(hooks.outdatedIds.size).toBe(0);
  });

  it('is read with the list, without the dialog, and counts the hooks whose catalog text is newer', async () => {
    fakeHooks('outdated');
    hooks.follow(null);

    await waitFor(() => expect(hooks.updates).toBe(1));
    expect(hooks.outdated.map((entry) => [entry.seed, entry.stale])).toEqual([['memory-save', true]]);
  });

  it('says an edited hook has an update too, but not a stale one', async () => {
    fakeHooks('outdatedEdited');
    hooks.follow(null);

    await waitFor(() => expect(hooks.updates).toBe(1));
    expect(hooks.outdated.map((entry) => [entry.seed, entry.stale])).toEqual([['memory-save', false]]);
  });

  it('counts nothing when every hook is current or only missing', async () => {
    fakeHooks('ok');
    hooks.follow(null);

    await waitFor(() => expect(hooks.family).not.toBeNull());
    expect(hooks.updates).toBe(0);
  });

  it('follows the update away once the hook is updated', async () => {
    fakeHooks('outdated');
    hooks.follow(null);
    await waitFor(() => expect(hooks.updates).toBe(1));

    await hooks.applyFamily(['memory-save']);

    expect(hooks.updates).toBe(0);
  });
});

describe('a family hook with an update waiting in the list', () => {
  it('says so with a chip next to its name, and only on that row', async () => {
    fakeHooks('outdated');
    await showHooks();

    await waitFor(() => expect(within(rowOf(SAVE)).getByText('Update available')).toBeInTheDocument());
    expect(within(rowOf(SAVE)).getByText('Active')).toBeInTheDocument();
    expect(within(rowOf('Search Harness Memory first')).queryByText('Update available')).not.toBeInTheDocument();
    expect(screen.getAllByText('Update available')).toHaveLength(1);
  });

  it('shows no chip while every family hook is current', async () => {
    fakeHooks('ok');
    await showHooks();

    await waitFor(() => expect(hooks.family).not.toBeNull());
    expect(screen.queryByText('Update available')).not.toBeInTheDocument();
  });
});

describe('the panel of a family hook with an update waiting', () => {
  it('offers Update for a hook on an earlier catalog version, and updates it in place', async () => {
    const { invoke } = fakeHooks('outdated');
    await showHooks();
    await waitFor(() => expect(within(rowOf(SAVE)).getByText('Update available')).toBeInTheDocument());

    await fireEvent.click(rowOf(SAVE));
    const panel = await screen.findByRole('region', { name: SAVE });
    expect(within(panel).getByText(NOTICE)).toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Review update' })).not.toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Update' }));

    await waitFor(() => expect(within(panel).queryByText(NOTICE)).not.toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['memory-save'] }]);
    expect(within(rowOf(SAVE)).queryByText('Update available')).not.toBeInTheDocument();
  });

  it('offers Review update for a hook the user edited, which opens the dialog that asks', async () => {
    const { invoke } = fakeHooks('outdatedEdited');
    await showHooks();
    await waitFor(() => expect(within(rowOf(SAVE)).getByText('Update available')).toBeInTheDocument());

    await fireEvent.click(rowOf(SAVE));
    const panel = await screen.findByRole('region', { name: SAVE });
    expect(within(panel).getByText(NOTICE)).toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Review update' }));

    expect(await screen.findByRole('dialog', { name: 'Harness family hooks' })).toBeInTheDocument();
    expect(calls(invoke, 'hooks_family_apply')).toEqual([]);
  });

  it('has no notice on a hook that is current', async () => {
    fakeHooks('outdated');
    await showHooks();
    await waitFor(() => expect(hooks.updates).toBe(1));

    await fireEvent.click(rowOf('Search Harness Memory first'));
    const panel = await screen.findByRole('region', { name: 'Search Harness Memory first' });

    expect(within(panel).queryByText(NOTICE)).not.toBeInTheDocument();
  });
});
