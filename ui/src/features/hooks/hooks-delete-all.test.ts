import { fireEvent, screen, waitFor } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { IpcFailure } from '$lib/ipc/transport';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, fakeHooks, PROJECT, registerHooksHooks, rowOf, showHooks } from './hooks-test-support';

registerHooksHooks();

const LINT = 'Run lint after edits';

async function openMenu() {
  await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));

  return screen.findByRole('menuitem', { name: 'Delete all hooks…' });
}

describe('deleting all hooks', () => {
  it('asks with the number, whose hooks go and where the family hooks come back, deletes by project, closes the panel and says how many went', async () => {
    const { invoke } = fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf(LINT));
    await screen.findByRole('region', { name: LINT });

    await fireEvent.click(await openMenu());

    expect(await screen.findByText('Delete the 4 hooks of Global and demo?')).toBeInTheDocument();
    expect(screen.getByText(/can add again from "Harness family hooks"\. Hooks of other projects and the team's stay\. This cannot be undone\./)).toBeInTheDocument();
    expect(calls(invoke, 'hooks_delete_all')).toHaveLength(0);

    await fireEvent.click(screen.getByRole('button', { name: 'Delete all hooks' }));

    expect(await screen.findByText('No hooks yet')).toBeInTheDocument();
    expect(calls(invoke, 'hooks_delete_all')).toEqual([{ project: PROJECT }]);
    expect(viewState.hookId).toBeNull();
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(toasts.items.at(-1)).toMatchObject({ title: '4 hooks deleted', tone: 'success' });
  });

  it('keeps everything when it is cancelled', async () => {
    const { invoke } = fakeHooks();
    await showHooks();

    await fireEvent.click(await openMenu());
    await fireEvent.click(await screen.findByRole('button', { name: 'Keep them' }));

    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(calls(invoke, 'hooks_delete_all')).toHaveLength(0);
    expect(rowOf(LINT)).toBeInTheDocument();
  });

  it('says why it failed inside the dialog and keeps the hooks', async () => {
    const { invoke } = fakeHooks();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    invoke.mockImplementation(((name: CommandName, args: never) =>
      name === 'hooks_delete_all' ? Promise.reject(IpcFailure.from({ code: 'io', message: 'the folder is read only' })) : real(name as never, args)) as typeof ipc.invoke);
    await showHooks();

    await fireEvent.click(await openMenu());
    await fireEvent.click(await screen.findByRole('button', { name: 'Delete all hooks' }));

    expect(await screen.findByText('Could not delete the hooks: the folder is read only')).toBeInTheDocument();
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    expect(rowOf(LINT)).toBeInTheDocument();
  });

  it('is off when there are no hooks', async () => {
    fakeHooks('empty');
    await showHooks();
    await screen.findByText('No hooks yet');

    expect(await openMenu()).toBeDisabled();
  });
});
