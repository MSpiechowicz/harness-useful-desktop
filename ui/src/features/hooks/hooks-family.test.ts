import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { onboarding } from '../onboarding/onboarding-state.svelte';

import { hooks } from './hooks-data.svelte';
import { samplePrefs } from '../../../tests/helpers';

import { calls, fakeHooks, registerHooksHooks, rowOf, showHooks } from './hooks-test-support';

registerHooksHooks();

async function openFamily() {
  await fireEvent.click(await screen.findByRole('button', { name: 'More actions' }));
  await fireEvent.click(await screen.findByRole('menuitem', { name: 'Harness family hooks' }));

  return screen.findByRole('dialog', { name: 'Harness family hooks' });
}

const rowIn = (dialog: HTMLElement, name: string) => within(dialog).getByText(name).closest<HTMLElement>('div.py-4') as HTMLElement;

describe('The Harness Desktop MCP server hint', () => {
  afterEach(() => onboarding.dismiss());

  it('tells an empty view that agents add hooks through the app\'s MCP server, and opens the Recommended setup from there', async () => {
    fakeHooks('empty');
    await showHooks();

    const hint = await screen.findByRole('note');
    expect(hint).toHaveTextContent('Agents add hooks through the Harness Desktop MCP server.');

    await fireEvent.click(within(hint).getByRole('button', { name: 'Open Recommended setup' }));

    expect(onboarding.rerun).toBe(true);
    expect(onboarding.step).toBe('setup');
  });

  it('is also in the family dialog, and closes it on the way out', async () => {
    fakeHooks();
    await showHooks();
    const dialog = await openFamily();

    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Open Recommended setup' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Harness family hooks' })).not.toBeInTheDocument());
    expect(onboarding.step).toBe('setup');
  });

  it('stays out of the way once a harness has the server', async () => {
    fakeHooks('empty', { on: true }, true);
    await showHooks();

    await screen.findByText('No hooks yet');
    await waitFor(() => expect(hooks.desktopMcp).toBe('registered'));
    expect(screen.queryByRole('note')).not.toBeInTheDocument();
  });
});

describe('Harness family hooks', () => {
  it('lists the catalog with what each hook needs, and installs a missing one in place', async () => {
    const { invoke } = fakeHooks();
    await showHooks();
    const dialog = await openFamily();

    await waitFor(() => expect(within(rowIn(dialog, 'Harness Memory')).getByText('Installed')).toBeInTheDocument());
    expect(within(rowIn(dialog, 'Harness Memory - save')).getByText('Installed')).toBeInTheDocument();
    expect(within(rowIn(dialog, 'Harness Skills')).getByRole('button', { name: /^Install / })).toBeEnabled();
    expect(within(rowIn(dialog, 'Harness Code')).getByRole('button', { name: 'Install Harness Code' })).toBeEnabled();
    // One action per row and a footer that only closes: nothing installs the rest in one go.
    expect(within(dialog).queryByRole('button', { name: /Add all/ })).not.toBeInTheDocument();
    expect(within(dialog).getAllByRole('button', { name: /^Install / })).toHaveLength(5);

    await fireEvent.click(within(rowIn(dialog, 'Harness Dashboard')).getByRole('button', { name: /^Install / }));

    await waitFor(() => expect(within(rowIn(dialog, 'Harness Dashboard')).getByText('Installed')).toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['dashboard-limits'] }]);
    expect(within(rowOf('Harness Dashboard')).getByText('Active')).toBeInTheDocument();
    expect(toasts.items).toContainEqual(expect.objectContaining({ title: 'Family hooks ready', description: '1 Harness family hook added.' }));
    await waitFor(() => expect(within(dialog).getAllByRole('button', { name: /^Install / })).toHaveLength(4));
  });

  it('numbers the steps in the order the agent works through them and says how often each hook runs', async () => {
    fakeHooks();
    await showHooks();
    const dialog = await openFamily();

    const steps = ['Harness Dashboard', 'Git branch', 'Harness Skills', 'Harness Memory', 'Harness Knowledge', 'Harness Code'];

    for (const [index, name] of steps.entries()) {
      expect(within(await waitFor(() => rowIn(dialog, name))).getByText(`Step ${index + 1}`)).toBeInTheDocument();
    }

    // The follow-up is not a step, and every row keeps the same shape.
    expect(within(rowIn(dialog, 'Harness Memory - save')).queryByText(/^Step \d/)).not.toBeInTheDocument();
    expect(within(dialog).getAllByText(/^Step \d/)).toHaveLength(steps.length);

    expect(within(rowIn(dialog, 'Git branch')).getByText('Harness Memory · Once per chat')).toBeInTheDocument();
    expect(within(rowIn(dialog, 'Harness Skills')).getByText('Harness Skills · Every message')).toBeInTheDocument();
    expect(within(rowIn(dialog, 'Harness Knowledge')).getByText('Harness Knowledge · Every message')).toBeInTheDocument();
    expect(within(rowIn(dialog, 'Harness Memory - save')).getByText('Harness Memory · After each turn')).toBeInTheDocument();
    expect(within(dialog).getAllByText('Harness Memory · Every message')).toHaveLength(1);
  });

  it('leaves out a hook the catalog merged into another once no file of it is left', async () => {
    fakeHooks();
    await showHooks();
    const dialog = await openFamily();

    await waitFor(() => rowIn(dialog, 'Harness Memory'));

    expect(within(dialog).queryByText('Search Harness Memory first')).not.toBeInTheDocument();
    expect(within(dialog).queryByText(/Merged, will be removed/)).not.toBeInTheDocument();
  });

  it('lists a merged hook while its untouched file is left, and removes the file on update', async () => {
    const { invoke } = fakeHooks('merged');
    await showHooks();
    const dialog = await openFamily();

    const merged = await waitFor(() => rowIn(dialog, 'Search Harness Memory first'));
    expect(within(merged).getByText('Harness Memory · Merged, will be removed')).toBeInTheDocument();
    expect(within(merged).queryByText(/^Step \d/)).not.toBeInTheDocument();

    await fireEvent.click(within(merged).getByRole('button', { name: /^Update / }));

    await waitFor(() => expect(within(dialog).queryByText('Search Harness Memory first')).not.toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['memory-search'] }]);
  });

  it('sends the user to the Ecosystem page when Harness Skills are not installed', async () => {
    fakeHooks('noSkills');
    await showHooks();
    const dialog = await openFamily();

    await fireEvent.click(await within(await waitFor(() => rowIn(dialog, 'Harness Skills'))).findByRole('button', { name: 'Open Ecosystem' }));

    expect(nav.current).toBe('ecosystem');
  });

  it('updates a family hook whose text the catalog has since changed', async () => {
    fakeHooks('outdated');
    await showHooks();
    const dialog = await openFamily();

    const row = await waitFor(() => {
      const found = rowIn(dialog, 'Harness Memory - save');
      expect(within(found).getByRole('button', { name: /^Update / })).toBeInTheDocument();

      return found;
    });
    await fireEvent.click(within(row).getByRole('button', { name: /^Update / }));

    await waitFor(() => expect(within(rowIn(dialog, 'Harness Memory - save')).getByText('Installed')).toBeInTheDocument());
    expect(screen.queryByRole('dialog', { name: 'Replace your edited version?' })).not.toBeInTheDocument();
  });

  it('asks before an update replaces a family hook the user edited, and leaves it alone when they keep it', async () => {
    const { invoke } = fakeHooks('outdatedEdited');
    await showHooks();
    const dialog = await openFamily();

    const update = await waitFor(() => within(rowIn(dialog, 'Harness Memory - save')).getByRole('button', { name: /^Update / }));
    await fireEvent.click(update);

    const confirm = await screen.findByRole('dialog', { name: 'Replace your edited version?' });
    await fireEvent.click(within(confirm).getByRole('button', { name: 'Keep my version' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Replace your edited version?' })).not.toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([]);

    await fireEvent.click(within(rowIn(dialog, 'Harness Memory - save')).getByRole('button', { name: /^Update / }));
    await fireEvent.click(within(await screen.findByRole('dialog', { name: 'Replace your edited version?' })).getByRole('button', { name: 'Update hook' }));

    await waitFor(() => expect(within(rowIn(dialog, 'Harness Memory - save')).getByText('Installed')).toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['memory-save'] }]);
  });

  it('updates a family hook still on an earlier catalog version by itself while automatic updates are on, and says so', async () => {
    const { invoke } = fakeHooks('outdated');
    prefs.value = samplePrefs({ autoUpdateExtensions: true });

    await hooks.updateStaleFamily();

    expect(calls(invoke, 'hooks_family_update_stale')).toHaveLength(1);
    expect(((await invoke('hooks_family', undefined)) as Array<{ seed: string }>).find((row) => row.seed === 'memory-save')).toMatchObject({ status: 'installed', stale: false });
    expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual(['1 family hook updated', 'Automatic updates brought it to the latest Harness family version.']);
  });

  it('leaves an outdated family hook for the dialog while automatic updates are off', async () => {
    const { invoke } = fakeHooks('outdated');
    prefs.value = samplePrefs({ autoUpdateExtensions: false });

    await hooks.updateStaleFamily();

    expect(calls(invoke, 'hooks_family_update_stale')).toEqual([]);
    expect(calls(invoke, 'hooks_family_apply')).toEqual([]);
  });

  it('installs a hook from an empty folder one row at a time', async () => {
    const { invoke } = fakeHooks('empty');
    await showHooks();

    expect(await screen.findByText('No hooks yet')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Harness family hooks' }));
    const dialog = await screen.findByRole('dialog', { name: 'Harness family hooks' });

    await fireEvent.click(within(await waitFor(() => rowIn(dialog, 'Harness Memory - save'))).getByRole('button', { name: /^Install / }));

    await waitFor(() => expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['memory-save'] }]));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }));
    await waitFor(() => expect(screen.queryByText('No hooks yet')).not.toBeInTheDocument());
    expect(within(rowOf('Harness Memory - save')).getByText('Active')).toBeInTheDocument();
  });

  it('installs the recall of Harness Knowledge and says it sends the message to that tool', async () => {
    const { invoke } = fakeHooks();
    await showHooks();
    const dialog = await openFamily();

    const recall = await waitFor(() => rowIn(dialog, 'Harness Knowledge'));
    expect(within(dialog).getByText(/^Recall sends each message you write to Harness Knowledge on this computer/)).toBeInTheDocument();

    await fireEvent.click(within(recall).getByRole('button', { name: /^Install / }));

    await waitFor(() => expect(within(rowIn(dialog, 'Harness Knowledge')).getByText('Installed')).toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['knowledge-first'] }]);
  });

  it('installs a member that is missing, right in the row, after the user confirms', async () => {
    fakeHooks('noDashboard');
    await showHooks();
    const dialog = await openFamily();

    const install = await within(dialog).findByRole('button', { name: 'Install Harness Dashboard' });
    await fireEvent.click(install);

    const confirm = await screen.findByRole('dialog', { name: 'Install Harness Dashboard?' });
    await fireEvent.click(within(confirm).getByRole('button', { name: 'Install Harness Dashboard' }));

    await waitFor(() => expect(within(rowIn(dialog, 'Harness Dashboard')).getByText('Installed')).toBeInTheDocument(), { timeout: 5000 });
    expect(within(rowOf('Harness Dashboard')).getByText('Active')).toBeInTheDocument();
  });
});
