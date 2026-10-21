import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { nav } from '$lib/stores/nav.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { hooks } from './hooks-data.svelte';
import { calls, fakeHooks, pick, PROJECT, registerHooksHooks, rowOf, showHooks } from './hooks-test-support';

registerHooksHooks();

const LINT = 'Run lint after edits';

describe('Hooks view', () => {
  it('lists the hooks in one table, each with its scope and its status, and opens no panel by itself', async () => {
    fakeHooks();
    await showHooks();

    const grid = screen.getByRole('grid', { name: 'Hooks' });
    expect(within(grid).queryByRole('columnheader', { name: 'This project' })).not.toBeInTheDocument();
    expect(within(grid).queryByText('2 of 64')).not.toBeInTheDocument();
    expect(within(grid).getAllByRole('columnheader').map((cell) => cell.textContent?.trim())).toEqual(['Hook', 'Scope', 'When', 'Does', 'Status', 'On']);
    expect(within(rowOf('Search Harness Memory first')).getByText('Global')).toBeInTheDocument();
    expect(within(rowOf(LINT)).getByText('demo')).toBeInTheDocument();
    expect(within(rowOf('Remind about tests')).getByText('demo')).toBeInTheDocument();

    expect(within(rowOf('Search Harness Memory first')).getByText('Active')).toBeInTheDocument();
    expect(within(rowOf('Search Harness Memory first')).getByText('Session start')).toBeInTheDocument();
    expect(within(rowOf(LINT)).getByText('Pending approval')).toBeInTheDocument();
    expect(within(rowOf(LINT)).getByText('edit|write')).toBeInTheDocument();
    expect(within(rowOf('Remind about tests')).getByText('Off')).toBeInTheDocument();

    expect(screen.queryByRole('region', { name: LINT })).not.toBeInTheDocument();
  });

  it('says what waits for approval in the header and how many hooks there are once, in the row of the filters', async () => {
    fakeHooks();
    await showHooks();

    expect(hooks.pending).toBe(1);
    expect(screen.getByText('1 to approve')).toBeInTheDocument();
    expect(screen.getAllByText('4 hooks')).toHaveLength(1);

    await pick('Status: All', /^Off/);

    expect(await screen.findByText('1 hook')).toBeInTheDocument();
    expect(screen.queryByText('4 hooks')).not.toBeInTheDocument();
  });

  it('counts the hooks in the menus of the two filters', async () => {
    fakeHooks();
    await showHooks();

    expect(screen.getByRole('button', { name: 'When: All' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Status: All' })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'When: All' }));
    const when = await screen.findByRole('menu', { name: 'When' });
    expect(within(when).getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['All 4', 'Session start 1', 'Prompt submit 1', 'After tool call 1', 'Turn end 1', 'Session end 0']);
    expect(within(when).getByRole('menuitemradio', { name: /^All/ })).toHaveAttribute('aria-checked', 'true');
    await fireEvent.keyDown(when, { key: 'Escape' });

    await fireEvent.click(screen.getByRole('button', { name: 'Status: All' }));
    const status = await screen.findByRole('menu', { name: 'Status' });
    expect(within(status).getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['All 4', 'Active 2', 'Pending approval 1', 'Off 1', 'Paused 0', 'Invalid 0']);
  });

  it('narrows the list by event, by status, and by both with the search, and says when nothing is left', async () => {
    fakeHooks();
    await showHooks();

    await pick('Status: All', /^Pending approval/);
    expect(screen.getByRole('button', { name: 'Status: Pending approval' })).toBeInTheDocument();
    expect(screen.getByText(LINT)).toBeInTheDocument();
    expect(screen.queryByText('Search Harness Memory first')).not.toBeInTheDocument();
    expect(viewState.hooksStatus).toBe('pending');

    await pick('When: All', /^Session end/);
    expect(await screen.findByText('No hooks match these filters.')).toBeInTheDocument();

    await pick('When: Session end', /^After tool call/);
    expect(screen.getByText(LINT)).toBeInTheDocument();
    expect(screen.queryByText('Remind about tests')).not.toBeInTheDocument();

    await pick('Status: Pending approval', /^All/);
    expect(screen.getByText(LINT)).toBeInTheDocument();
    expect(screen.queryByText('Search Harness Memory first')).not.toBeInTheDocument();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search hooks' }), { target: { value: 'tests' } });
    expect(await screen.findByText('No hook matches your search.')).toBeInTheDocument();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search hooks' }), { target: { value: 'lint' } });
    expect(screen.getByText(LINT)).toBeInTheDocument();
  });

  it('clears a filter with the x in its button, and only that one', async () => {
    fakeHooks();
    await showHooks();

    expect(screen.queryByRole('button', { name: 'Clear the When filter' })).not.toBeInTheDocument();

    await pick('When: All', /^Prompt submit/);
    await pick('Status: All', /^Off/);
    expect(screen.getByRole('button', { name: 'When: Prompt submit' })).toBeInTheDocument();
    expect(screen.getByText('Remind about tests')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Status filter' }));
    expect(screen.getByRole('button', { name: 'Status: All' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'When: Prompt submit' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Clear the Status filter' })).not.toBeInTheDocument();
    expect(viewState.hooksStatus).toBe('all');
    expect(viewState.hooksWhen).toBe('promptSubmit');

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the When filter' }));
    expect(screen.getByText(LINT)).toBeInTheDocument();
    expect(screen.getByText('Search Harness Memory first')).toBeInTheDocument();
  });

  it('remembers the filters when the view is left and shown again', async () => {
    fakeHooks();
    await showHooks();

    await pick('When: All', /^Session start/);
    cleanup();
    await showHooks();

    expect(screen.getByRole('button', { name: 'When: Session start' })).toBeInTheDocument();
  });

  it('opens a hook on a click, with the way to read what it does', async () => {
    fakeHooks();
    await showHooks();

    await fireEvent.click(rowOf('Search Harness Memory first'));

    const panel = await screen.findByRole('region', { name: 'Search Harness Memory first' });
    expect(within(panel).getByText('Before you start, search Harness Memory for notes about this task (note_search) and check the saved scripts (script_list). Reuse what fits. Pass project_path on every Harness Memory call: the absolute path of the repository you are working in (its git root), so nothing is read from or saved to another repository.')).toBeInTheDocument();
    expect(within(panel).getByText('Harness family')).toBeInTheDocument();
    expect(viewState.hookId).not.toBeNull();
  });

  it('switches a hook off and on from its row', async () => {
    const { invoke } = fakeHooks();
    await showHooks();

    await fireEvent.click(within(rowOf('Search Harness Memory first')).getByRole('switch', { name: 'Turn off Search Harness Memory first' }));

    await waitFor(() => expect(within(rowOf('Search Harness Memory first')).getByText('Off')).toBeInTheDocument());
    expect(calls(invoke, 'hook_save')).toHaveLength(1);
    expect(screen.queryByRole('region', { name: 'Search Harness Memory first' })).not.toBeInTheDocument();

    await fireEvent.click(within(rowOf('Search Harness Memory first')).getByRole('switch', { name: 'Turn on Search Harness Memory first' }));
    await waitFor(() => expect(within(rowOf('Search Harness Memory first')).getByText('Active')).toBeInTheDocument());
  });

  it('keeps the switch of a hook that waits for approval off, since a save would approve it unseen', async () => {
    const { invoke } = fakeHooks();
    await showHooks();

    const toggle = within(rowOf(LINT)).getByRole('switch');
    expect(toggle).toBeDisabled();
    expect(toggle).toHaveAttribute('aria-checked', 'false');

    await fireEvent.click(rowOf(LINT));
    const panel = await screen.findByRole('region', { name: LINT });

    expect(within(panel).getByRole('switch')).toBeDisabled();
    expect(within(panel).getByRole('button', { name: 'Edit' })).toBeDisabled();
    expect(calls(invoke, 'hook_save')).toHaveLength(0);
  });
});

describe('Hook details', () => {
  it('names the whole folder of a project hook, not just its short name', async () => {
    fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf(LINT));

    const details = within(await screen.findByRole('region', { name: LINT })).getByText('Details').closest('section') as HTMLElement;

    expect(within(details).getByText('Project')).toBeInTheDocument();
    expect(within(details).getByText(PROJECT)).toBeInTheDocument();
  });

  it('credits the Harness family only while the version on disk is one the user approved', async () => {
    const { state } = fakeHooks();
    await showHooks();
    const seeded = 'Search Harness Memory first';

    await fireEvent.click(rowOf(seeded));
    expect(within(await screen.findByRole('region', { name: seeded })).getByText('Harness family')).toBeInTheDocument();

    // An agent rewrites the family hook: it now waits for approval, and it is the agent's version.
    state.agentChanges(viewState.hookId ?? '');
    await hooks.refresh();

    const panel = await screen.findByRole('region', { name: `${seeded} v2` });
    expect(within(panel).queryByText('Harness family')).not.toBeInTheDocument();
    expect(within(panel).getByText('An agent')).toBeInTheDocument();
  });

  it('spells out hidden characters in the name and the description an agent wrote', async () => {
    const { state } = fakeHooks();
    await showHooks();

    state.agentAdds('Run\u202E tests', 'Starts\u200B here.');
    await hooks.refresh();

    const name = 'Run⟨U+202E⟩ tests';
    await fireEvent.click(rowOf(name));
    const panel = await screen.findByRole('region', { name });

    expect(within(panel).getByText('⟨U+200B⟩')).toBeInTheDocument();
  });
});

describe('A save of a hook that waits for approval', () => {
  it('is refused by the backend and said in a toast, with the hook left as it was', async () => {
    fakeHooks();
    await showHooks();
    const pending = hooks.views.find((view) => view.status === 'pending') as NonNullable<(typeof hooks.views)[number]>;

    expect(await hooks.toggle(pending, true)).toBe(false);

    expect(toasts.items[0]).toMatchObject({ title: 'Could not switch the hook', description: 'this hook waits for your approval and cannot be saved' });
    expect(hooks.find(pending.id)?.status).toBe('pending');
  });
});

describe('Reviewing a hook an agent wrote', () => {
  it('shows what changed against the approved version, and nothing that stayed', async () => {
    fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf(LINT));

    const panel = await screen.findByRole('region', { name: LINT });
    const review = within(panel).getByRole('status', { name: '' });

    expect(within(review).getByText('An agent changed this hook')).toBeInTheDocument();
    expect(within(review).getByRole('group', { name: 'Changes to Tool filter' })).toHaveTextContent('edit|write');
    expect(within(review).getByRole('group', { name: 'Changes to Tool filter' })).toHaveTextContent('edit');
    expect(within(review).getByRole('figure', { name: 'Changes to Command' })).toHaveTextContent('bun run lint --quiet');
    expect(within(review).queryByRole('group', { name: 'Changes to Name' })).not.toBeInTheDocument();
  });

  it('runs the hook once the user approves exactly that version', async () => {
    const { invoke } = fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf(LINT));

    await fireEvent.click(within(await screen.findByRole('region', { name: LINT })).getByRole('button', { name: 'Approve' }));

    await waitFor(() => expect(within(rowOf(LINT)).getByText('Active')).toBeInTheDocument());
    await waitFor(() => expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual(['Run lint after edits approved', 'It runs after the agent uses a tool.']));
    expect(calls(invoke, 'hook_approve')).toEqual([{ id: expect.stringMatching(/^[0-9a-f]{32}$/), sha256: expect.stringMatching(/^[0-9a-f]{64}$/) }]);
    expect(hooks.pending).toBe(0);
    expect(within(screen.getByRole('region', { name: LINT })).queryByRole('button', { name: 'Approve' })).not.toBeInTheDocument();
  });

  it('puts an edited hook back as it was approved when the user rejects it', async () => {
    fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf(LINT));

    await fireEvent.click(within(await screen.findByRole('region', { name: LINT })).getByRole('button', { name: 'Reject and undo' }));

    await waitFor(() => expect(within(rowOf(LINT)).getByText('Active')).toBeInTheDocument());
    expect(within(rowOf(LINT)).getByText('edit')).toBeInTheDocument();
    expect(within(rowOf(LINT)).queryByText('edit|write')).not.toBeInTheDocument();
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Rejected. The hook is back as you approved it.'));
  });

  it('removes a hook that was never approved when the user rejects it', async () => {
    const { state } = fakeHooks();
    await showHooks();

    const added = state.agentAdds('Run the tests');
    await hooks.refresh();
    await fireEvent.click(await screen.findByText('Run the tests', { selector: '[data-row] td div' }));

    const panel = await screen.findByRole('region', { name: 'Run the tests' });
    expect(within(panel).getByText('An agent added this hook')).toBeInTheDocument();
    expect(hooks.pending).toBe(2);

    await fireEvent.click(within(panel).getByRole('button', { name: 'Reject and remove' }));

    await waitFor(() => expect(screen.queryByText('Run the tests', { selector: '[data-row] td div' })).not.toBeInTheDocument());
    expect(hooks.find(added.id)).toBeNull();
    expect(hooks.pending).toBe(1);
  });

  it('tells the user when the hook changed again before they decided', async () => {
    const { state } = fakeHooks();
    await showHooks();
    await fireEvent.click(rowOf(LINT));
    const panel = await screen.findByRole('region', { name: LINT });
    const open = viewState.hookId ?? '';

    // An agent rewrites the file after the list was read, and before the user's click.
    state.agentChanges(open);

    await fireEvent.click(within(panel).getByRole('button', { name: 'Approve' }));

    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('This hook changed in the meantime.'));
  });
});

describe('Hooks that wait for Harness Memory', () => {
  it('say so above the list while it is off, and the way to Settings', async () => {
    const memory = { on: false };
    fakeHooks('ok', memory);
    prefs.value = { ...(prefs.value as NonNullable<typeof prefs.value>), memoryEnabled: false };
    await showHooks();

    const notice = screen.getByText('Harness Memory is off').closest<HTMLElement>('[role="status"]') as HTMLElement;
    expect(within(notice).getByText('Harness Memory is off')).toBeInTheDocument();
    expect(within(notice).getByText('2 hooks use Harness Memory and wait until it is back on.')).toBeInTheDocument();
    expect(within(rowOf('Save what you learned')).getByText('Paused')).toBeInTheDocument();

    await fireEvent.click(within(notice).getByRole('button', { name: 'Open Settings' }));

    expect(nav.current).toBe('settings');
    expect(viewState.settingsTarget).toBe('memory');
    prefs.value = { ...(prefs.value as NonNullable<typeof prefs.value>), memoryEnabled: true };
  });

  it('are quiet while Harness Memory is on', async () => {
    fakeHooks();
    await showHooks();

    expect(screen.queryByText('Harness Memory is off')).not.toBeInTheDocument();
  });
});

describe('The states of the view', () => {
  it('explains an empty folder and offers a first hook and the Harness family hooks', async () => {
    fakeHooks('empty');
    await showHooks();

    expect(await screen.findByText('No hooks yet')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'New hook' }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole('button', { name: 'Harness family hooks' }).length).toBeGreaterThan(0);
  });

  it('says why a folder is not read, with a way to try again', async () => {
    fakeHooks('problem');
    await showHooks();

    expect(await screen.findByText('Hooks are not available')).toBeInTheDocument();
    expect(screen.getByText(/can be changed by other users/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('says when the hooks cannot be read at all', async () => {
    fakeHooks('failing');
    await showHooks();

    expect(await screen.findByText('Could not read the hooks')).toBeInTheDocument();
    expect(screen.getByText('The hooks folder could not be read.')).toBeInTheDocument();
  });

  it('lists a file that breaks the format apart from the hooks, and opens it with the reason', async () => {
    fakeHooks('invalid');
    await showHooks();

    const row = screen.getByText('Format on save', { selector: '[data-row] td div' }).closest<HTMLElement>('[data-row]') as HTMLElement;
    expect(within(row).getByText('Invalid')).toBeInTheDocument();
    expect(within(row).getByRole('switch')).toBeDisabled();

    await fireEvent.click(row);
    const panel = await screen.findByRole('region', { name: 'Format on save' });

    expect(within(panel).getByText('This file cannot run')).toBeInTheDocument();
    expect(within(panel).getByText('The action `followUp` cannot run at `sessionStart`.')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Edit' })).toBeDisabled();
  });
});
