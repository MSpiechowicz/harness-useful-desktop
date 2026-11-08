import { screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { noteList } from '$lib/stores/memoryNotes.svelte';
import { scriptList } from '$lib/stores/memoryScripts.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { openSync, SYNC_VIEWS, syncButton, withCopyPrefs } from './memory-sync-support';
import { calls, registerMemoryHooks } from './memory-test-support';

registerMemoryHooks();

describe.each(SYNC_VIEWS)('the Sync button of the $name view', (view) => {
  const reread = () => (view.name === 'notes' ? noteList.refresh() : scriptList.refresh());

  it('sits next to the main action while the entries are read from the remote and this computer, the remote is reachable and a copy is on', async () => {
    withCopyPrefs(true, false);
    await view.open();

    const sync = await screen.findByRole('button', { name: 'Synchronize…' });
    const add = screen.getByRole('button', { name: view.name === 'notes' ? 'Add note' : 'Add script' });

    expect(sync.parentElement).toBe(add.parentElement);
  });

  it.each(['remote', 'local'] as const)('is not offered while the entries are kept in %s only', async (place) => {
    withCopyPrefs(true, true);
    const { invoke } = await view.open({}, place);

    await waitFor(() => expect(calls(invoke, 'memory_remote_get')).not.toHaveLength(0));
    await screen.findByRole('grid');
    expect(syncButton()).toBeNull();
  });

  it('is not offered while neither copy is turned on, and follows the switches', async () => {
    withCopyPrefs(false, false);
    await view.open();

    await screen.findByRole('grid');
    expect(syncButton()).toBeNull();

    prefs.value = { ...prefs.value!, memoryCopyToLocal: true };

    expect(await screen.findByRole('button', { name: 'Synchronize…' })).toBeInTheDocument();
  });

  it('is not offered while the installed Harness Memory cannot read both places', async () => {
    withCopyPrefs(true, true);
    await view.open({ teamAndLocalSupported: false, scriptsTeamAndLocalSupported: false });

    await screen.findByRole('grid');
    expect(syncButton()).toBeNull();
  });

  it('is not offered while the installed Harness Memory cannot copy between the two', async () => {
    withCopyPrefs(true, true);
    await view.open({ notesSyncSupported: false, scriptsSyncSupported: false });

    await screen.findByRole('grid');
    expect(syncButton()).toBeNull();
  });

  it('is hidden while the remote cannot be reached, and returns when it is back', async () => {
    withCopyPrefs(true, true);
    const { outage } = await view.open();
    await screen.findByRole('button', { name: 'Synchronize…' });

    outage.down = true;
    await reread();

    await waitFor(() => expect(syncButton()).toBeNull());

    outage.down = false;
    await reread();

    expect(await screen.findByRole('button', { name: 'Synchronize…' })).toBeInTheDocument();
  });

  it('opens the dialog on the copy that is on when only one is', async () => {
    withCopyPrefs(false, true);
    await view.open();
    await openSync();

    const dialog = screen.getByRole('dialog');

    expect(await within(dialog).findByText(view.name === 'notes' ? 'Copy remote notes to this computer?' : 'Copy remote scripts to this computer?')).toBeInTheDocument();
    expect(within(dialog).queryByRole('radiogroup', { name: 'Direction' })).toBeNull();
  });

  it('offers both directions, to remote first, when both copies are on', async () => {
    withCopyPrefs(true, true);
    await view.open();
    await openSync();

    const choice = within(screen.getByRole('dialog')).getByRole('radiogroup', { name: 'Direction' });
    const radios = within(choice).getAllByRole('radio');

    expect(radios.map((radio) => radio.textContent?.trim())).toEqual(['To remote', 'To this computer']);
    expect(radios[0]).toHaveAttribute('aria-checked', 'true');
  });
});
