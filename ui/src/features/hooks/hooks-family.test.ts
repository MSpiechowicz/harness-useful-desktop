import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import { toasts } from '$lib/stores/toasts.svelte';

import { onboarding } from '../onboarding/onboarding-state.svelte';

import { hooks } from './hooks-data.svelte';
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
  it('lists the catalog with what each hook needs, and adds a missing one in place', async () => {
    const { invoke } = fakeHooks();
    await showHooks();
    const dialog = await openFamily();

    await waitFor(() => expect(within(rowIn(dialog, 'Search Harness Memory first')).getByText('Installed')).toBeInTheDocument());
    expect(within(rowIn(dialog, 'Save what you learned')).getByText('Installed')).toBeInTheDocument();
    expect(within(dialog).getByText('Harness Dashboard · Session start')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add the missing hook' })).toBeEnabled();

    await fireEvent.click(within(rowIn(dialog, 'Check limits before long work')).getByRole('button', { name: 'Add' }));

    await waitFor(() => expect(within(rowIn(dialog, 'Check limits before long work')).getByText('Installed')).toBeInTheDocument());
    expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['dashboard-limits'] }]);
    expect(within(rowOf('Check limits before long work')).getByText('Active')).toBeInTheDocument();
    expect(toasts.items.map((toast) => toast.title)).toContain('1 family hook ready');
    expect(within(dialog).getByRole('button', { name: 'Add all' })).toBeDisabled();
  });

  it('updates a family hook whose text the catalog has since changed', async () => {
    fakeHooks('outdated');
    await showHooks();
    const dialog = await openFamily();

    const row = await waitFor(() => {
      const found = rowIn(dialog, 'Save what you learned');
      expect(within(found).getByText('Outdated')).toBeInTheDocument();

      return found;
    });
    await fireEvent.click(within(row).getByRole('button', { name: 'Update' }));

    await waitFor(() => expect(within(rowIn(dialog, 'Save what you learned')).getByText('Installed')).toBeInTheDocument());
  });

  it('adds every missing family hook with one button, from an empty folder', async () => {
    const { invoke } = fakeHooks('empty');
    await showHooks();

    expect(await screen.findByText('No hooks yet')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Harness family hooks' }));
    const dialog = await screen.findByRole('dialog', { name: 'Harness family hooks' });

    await fireEvent.click(await within(dialog).findByRole('button', { name: 'Add all 3 missing hooks' }));

    await waitFor(() => expect(calls(invoke, 'hooks_family_apply')).toEqual([{ seeds: ['memory-search', 'memory-save', 'dashboard-limits'] }]));
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
