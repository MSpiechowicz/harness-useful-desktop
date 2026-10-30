import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

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

    await waitFor(() => expect(within(rowIn(dialog, 'Search Harness Memory first')).getByText('Installed')).toBeInTheDocument());
    expect(within(rowIn(dialog, 'Save what you learned')).getByText('Installed')).toBeInTheDocument();
    expect(within(rowIn(dialog, 'Check Useful Skills first')).getByRole('button', { name: /^Install / })).toBeEnabled();
    expect(within(dialog).getByText('Harness Dashboard · Session start')).toBeInTheDocument();
    expect(within(rowIn(dialog, 'Use the code graph first')).getByRole('button', { name: 'Install Use the code graph first' })).toBeEnabled();
    expect(within(dialog).getAllByText('Harness Memory · Prompt submit')).toHaveLength(3);
    // One action per row and a footer that only closes: nothing installs the rest in one go.
    expect(within(dialog).queryByRole('button', { name: /Add all/ })).not.toBeInTheDocument();
    expect(within(dialog).getAllByRole('button', { name: /^Install / })).toHaveLength(5);

    await fireEvent.click(within(rowIn(dialog, 'Check limits before long work')).getByRole('button', { name: /^Install / }));

    await waitFor(() => expect(within(rowIn(dialog, 'Check limits before long work')).getByText('Installed')).toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['dashboard-limits'] }]);
    expect(within(rowOf('Check limits before long work')).getByText('Active')).toBeInTheDocument();
    expect(toasts.items.map((toast) => toast.title)).toContain('1 family hook ready');
    await waitFor(() => expect(within(dialog).getAllByRole('button', { name: /^Install / })).toHaveLength(4));
  });

  it('updates a family hook whose text the catalog has since changed', async () => {
    fakeHooks('outdated');
    await showHooks();
    const dialog = await openFamily();

    const row = await waitFor(() => {
      const found = rowIn(dialog, 'Save what you learned');
      expect(within(found).getByRole('button', { name: /^Update / })).toBeInTheDocument();

      return found;
    });
    await fireEvent.click(within(row).getByRole('button', { name: /^Update / }));

    await waitFor(() => expect(within(rowIn(dialog, 'Save what you learned')).getByText('Installed')).toBeInTheDocument());
    expect(screen.queryByRole('dialog', { name: 'Replace your edited version?' })).not.toBeInTheDocument();
  });

  it('asks before an update replaces a family hook the user edited, and leaves it alone when they keep it', async () => {
    const { invoke } = fakeHooks('outdatedEdited');
    await showHooks();
    const dialog = await openFamily();

    const update = await waitFor(() => within(rowIn(dialog, 'Save what you learned')).getByRole('button', { name: /^Update / }));
    await fireEvent.click(update);

    const confirm = await screen.findByRole('dialog', { name: 'Replace your edited version?' });
    await fireEvent.click(within(confirm).getByRole('button', { name: 'Keep my version' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Replace your edited version?' })).not.toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([]);

    await fireEvent.click(within(rowIn(dialog, 'Save what you learned')).getByRole('button', { name: /^Update / }));
    await fireEvent.click(within(await screen.findByRole('dialog', { name: 'Replace your edited version?' })).getByRole('button', { name: 'Update hook' }));

    await waitFor(() => expect(within(rowIn(dialog, 'Save what you learned')).getByText('Installed')).toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['memory-save'] }]);
  });

  it('updates a family hook still on an earlier catalog version by itself while automatic updates are on, and says so', async () => {
    const { invoke } = fakeHooks('outdated');
    prefs.value = samplePrefs({ autoUpdateExtensions: true });

    await hooks.updateStaleFamily();

    expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['memory-save'] }]);
    expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual(['1 family hook updated', 'Automatic updates brought it to the latest Harness family version.']);
  });

  it('leaves an outdated family hook for the dialog while automatic updates are off', async () => {
    const { invoke } = fakeHooks('outdated');
    prefs.value = samplePrefs({ autoUpdateExtensions: false });

    await hooks.updateStaleFamily();

    expect(calls(invoke, 'hooks_family_apply')).toEqual([]);
  });

  it('installs a hook from an empty folder one row at a time', async () => {
    const { invoke } = fakeHooks('empty');
    await showHooks();

    expect(await screen.findByText('No hooks yet')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Harness family hooks' }));
    const dialog = await screen.findByRole('dialog', { name: 'Harness family hooks' });

    await fireEvent.click(within(await waitFor(() => rowIn(dialog, 'Save what you learned'))).getByRole('button', { name: /^Install / }));

    await waitFor(() => expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['memory-save'] }]));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Close' }));
    await waitFor(() => expect(screen.queryByText('No hooks yet')).not.toBeInTheDocument());
    expect(within(rowOf('Save what you learned')).getByText('Active')).toBeInTheDocument();
  });

  it('installs a member that is missing, right in the row, after the user confirms', async () => {
    fakeHooks('noDashboard');
    await showHooks();
    const dialog = await openFamily();

    const install = await within(dialog).findByRole('button', { name: 'Install Harness Dashboard' });
    await fireEvent.click(install);

    const confirm = await screen.findByRole('dialog', { name: 'Install Harness Dashboard?' });
    await fireEvent.click(within(confirm).getByRole('button', { name: 'Install Harness Dashboard' }));

    await waitFor(() => expect(within(rowIn(dialog, 'Check limits before long work')).getByText('Installed')).toBeInTheDocument(), { timeout: 5000 });
    expect(within(rowOf('Check limits before long work')).getByText('Active')).toBeInTheDocument();
  });
});
