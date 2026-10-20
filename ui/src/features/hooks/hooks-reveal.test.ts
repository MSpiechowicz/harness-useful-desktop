import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { HookDraft } from '$lib/ipc/bindings/HookDraft';
import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import type { MockHooks } from '$lib/ipc/mock/hooks-state';
import { nav } from '$lib/stores/nav.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { listed } from '../../../tests/doc-support';
import { viewKey } from './hook-format';
import { hooks } from './hooks-data.svelte';
import { calls, fakeHooks, OTHER_PROJECT, PROJECT, registerHooksHooks, rowOf, showHooks } from './hooks-test-support';
import HooksView from './HooksView.svelte';

registerHooksHooks();

const LINT = 'Run lint after edits';
const grid = () => screen.getByRole('grid', { name: 'Hooks' });

/** A hook of the user's in `project` (a folder, null for every project). */
function hookIn(state: MockHooks, name: string, project: string | null): string {
  const draft: HookDraft = { id: null, expectedSha256: null, name, description: null, scope: project === null ? 'global' : 'project', project, event: 'sessionStart', matcher: null, action: { type: 'addContext', text: 'Look at the notes.' }, enabled: true };

  return state.save(draft).id;
}

/** Answers `project_scopes` with `scopes` once `ready` resolves, as a slow Harness Memory does. */
function scopesWhen(invoke: ReturnType<typeof fakeHooks>['invoke'], ready: Promise<void>, scopes?: ProjectScope[]): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation(((name: CommandName, args: never) => {
    if (name !== 'project_scopes') {
      return real(name as never, args);
    }

    return ready.then(() => (scopes ?? real(name as never, args)));
  }) as typeof ipc.invoke);
}

/** Shows the view as soon as its hooks are read, not waiting for the projects. */
async function showBeforeProjects(): Promise<void> {
  nav.go('hooks');
  render(HooksView);
  await waitFor(() => expect(screen.queryByRole('status', { name: 'Loading hooks' })).not.toBeInTheDocument());
}

const deleteAll = () => screen.findByRole('menuitem', { name: 'Delete all hooks…' });

async function openMenu(): Promise<HTMLElement> {
  await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));

  return deleteAll();
}

describe('A Scope that is gone', () => {
  it('counts as All, so the hooks are listed instead of all hidden', async () => {
    viewState.hooksScope = 'repo:git:github.com/forgotten/project';
    fakeHooks();
    await showHooks();

    expect(screen.getByRole('button', { name: 'Scope: All' })).toBeInTheDocument();
    expect(listed(grid())).toContain(LINT);
    expect(screen.queryByText(/match these filters/)).not.toBeInTheDocument();
  });

  it('is not applied to a new hook either', async () => {
    viewState.hooksScope = 'repo:git:github.com/forgotten/project';
    fakeHooks();
    await showHooks();

    await fireEvent.click(screen.getAllByRole('button', { name: 'New hook' })[0]);

    expect(within(await screen.findByRole('dialog', { name: 'New hook' })).getByRole('button', { name: 'Applies to' })).toHaveTextContent('Global');
  });
});

describe('Delete all while the projects are still being read', () => {
  it('stays off until the open project is known, then names it and counts its hooks', async () => {
    const { invoke } = fakeHooks();
    let release = () => {};
    scopesWhen(invoke, new Promise<void>((resolve) => (release = resolve)));
    await showBeforeProjects();

    const item = await openMenu();
    expect(item).toBeDisabled();

    release();
    await waitFor(() => expect(item).toBeEnabled());
    await fireEvent.click(item);

    expect(await screen.findByText('Delete the 4 hooks of Global and demo?')).toBeInTheDocument();
  });

  it('stays off when the projects lack the open one, because the delete would still reach it', async () => {
    const { invoke } = fakeHooks();
    scopesWhen(invoke, Promise.resolve(), [{ path: OTHER_PROJECT, name: 'research', root: OTHER_PROJECT, key: `path:${OTHER_PROJECT}` }]);
    await showHooks();

    expect(await openMenu()).toBeDisabled();
    expect(calls(invoke, 'hooks_delete_all')).toHaveLength(0);
  });

  it('deletes with the path of the open project and names that same project', async () => {
    const { invoke } = fakeHooks();
    await showHooks();

    await fireEvent.click(await openMenu());
    expect(await screen.findByText('Delete the 4 hooks of Global and demo?')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Delete all hooks' }));

    await waitFor(() => expect(calls(invoke, 'hooks_delete_all')).toEqual([{ project: PROJECT }]));
  });
});

describe('A hook that opens', () => {
  it('lets go of the search and the Scope that hid it, and is shown selected', async () => {
    fakeHooks();
    await showHooks();
    viewState.hooksScope = 'global';
    viewState.hooksQuery = 'memory';
    await waitFor(() => expect(listed(grid())).not.toContain(LINT));

    const lint = hooks.views.find((view) => view.hook?.name === LINT);
    viewState.hookId = viewKey(lint as NonNullable<typeof lint>);

    await waitFor(() => expect(viewState.hooksScope).toBe('all'));
    expect(viewState.hooksQuery).toBe('');
    expect(rowOf(LINT)).toHaveAttribute('aria-selected', 'true');
  });

  it('turns the table to the page it is on', async () => {
    const { state } = fakeHooks();
    const ids: string[] = [];

    for (let at = 0; at < 60; at += 1) {
      ids.push(hookIn(state, `Hook number ${String(at).padStart(2, '0')}`, null));
    }

    await showHooks();
    await hooks.refresh();
    expect(viewState.hooksPage).toBe(0);

    viewState.hookId = ids[59];

    await waitFor(() => expect(viewState.hooksPage).toBe(1));
    expect(rowOf('Hook number 59')).toHaveAttribute('aria-selected', 'true');
  });

  it('shows a hook just added for a project the Scope filter does not show', async () => {
    fakeHooks();
    viewState.hooksScope = 'global';
    await showHooks();

    await fireEvent.click(screen.getAllByRole('button', { name: 'New hook' })[0]);
    const dialog = await screen.findByRole('dialog', { name: 'New hook' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Applies to' }));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: /^research/ }));
    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Mind the budget' } });
    await fireEvent.input(within(dialog).getByLabelText('Context'), { target: { value: 'Check the limits first.' } });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add hook' }));

    await waitFor(() => expect(viewState.hooksScope).toBe('all'));
    expect(rowOf('Mind the budget')).toHaveAttribute('aria-selected', 'true');
  });

  it('does not take the filters back every time the list is read again', async () => {
    fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf(LINT));
    await screen.findByRole('region', { name: LINT });

    viewState.hooksQuery = 'memory';
    await hooks.refresh();

    expect(viewState.hooksQuery).toBe('memory');
  });
});

describe('Editing a hook of a scope that holds the most', () => {
  it('can be saved unmoved, whenever the projects were read', async () => {
    const { state, invoke } = fakeHooks();
    let release = () => {};
    scopesWhen(invoke, new Promise<void>((resolve) => (release = resolve)));

    for (let at = 0; at < 64; at += 1) {
      hookIn(state, `Research hook ${String(at).padStart(2, '0')}`, OTHER_PROJECT);
    }

    await showBeforeProjects();
    await hooks.refresh();
    await fireEvent.click(rowOf('Research hook 00'));
    await fireEvent.click(await within(await screen.findByRole('region', { name: 'Research hook 00' })).findByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit hook' });

    release();
    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Applies to' })).toHaveTextContent('research'));

    expect(within(dialog).queryByText(/already holds/)).not.toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Save' })).toBeEnabled();
  });
});
