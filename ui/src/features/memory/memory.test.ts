import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

import type { ScriptDetail } from '$lib/ipc/bindings/ScriptDetail';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { sampleScripts } from '$lib/ipc/mock/memory-scripts-fixtures';
import { IpcFailure } from '$lib/ipc/transport';
import { nav } from '$lib/stores/nav.svelte';
import { toasts } from '$lib/stores/toasts.svelte';

import { calls, fakeMemory, PROJECT, registerMemoryHooks, rowButton, showMemory } from './memory-test-support';
import ScriptsView from './ScriptsView.svelte';

registerMemoryHooks();

describe('scripts table', () => {
  it('lists the scripts with their language, scope, place and runs, and asks for a page of them by project', async () => {
    const invoke = fakeMemory();
    await showMemory();

    expect(calls(invoke, 'memory_scripts_list')[0]).toEqual({ project: PROJECT, offset: 0, limit: 50, sort: 'updated', order: 'desc' });

    const open = within(rowButton('open-prs') as HTMLElement);
    expect(open.getByText('Bash')).toBeInTheDocument();
    expect(open.getByText('github.com/useful/harness-desktop')).toBeInTheDocument();
    expect(open.getByText('Team')).toBeInTheDocument();
    expect(open.getByText('14')).toBeInTheDocument();

    const global = within(rowButton('disk-hogs') as HTMLElement);
    expect(global.getByText('Global')).toBeInTheDocument();
    expect(global.getByText('Never')).toBeInTheDocument();
    expect(within(rowButton('ci-status') as HTMLElement).getByText('This computer')).toBeInTheDocument();
  });

  it('narrows the table by the search and says so when nothing matches', async () => {
    fakeMemory();
    await showMemory();
    const search = screen.getByRole('searchbox', { name: 'Search scripts' });

    await fireEvent.input(search, { target: { value: 'pull requests' } });
    await waitFor(() => expect(rowButton('ci-status')).toBeNull());
    expect(rowButton('open-prs')).not.toBeNull();
    // The count follows what the search leaves.
    expect(screen.getByText('1 script')).toBeInTheDocument();

    await fireEvent.input(search, { target: { value: 'zzz' } });
    expect(await screen.findByText('No script matches that search.')).toBeInTheDocument();
  });

  it('sorts by any column when its header is clicked, numbers and times from the biggest, and flips on a second click', async () => {
    const invoke = fakeMemory();
    await showMemory();
    const lastAsk = () => calls(invoke, 'memory_scripts_list').at(-1);

    for (const [header, sort, order] of [
      ['Name', 'name', 'asc'],
      ['Language', 'language', 'asc'],
      ['Scope', 'scope', 'asc'],
      ['Kept in', 'place', 'asc'],
      ['Runs', 'runs', 'desc'],
      ['Last run', 'lastRun', 'desc'],
    ] as const) {
      await fireEvent.click(screen.getByRole('button', { name: header }));

      await waitFor(() => expect(lastAsk()).toMatchObject({ sort, order, offset: 0 }));
      expect(screen.getByRole('columnheader', { name: header })).toHaveAttribute('aria-sort', order === 'asc' ? 'ascending' : 'descending');
    }

    await fireEvent.click(screen.getByRole('button', { name: 'Last run' }));
    await waitFor(() => expect(lastAsk()).toMatchObject({ sort: 'lastRun', order: 'asc' }));
  });

  it('turns the pages of a long list and says where the user is', async () => {
    const many = Array.from({ length: 120 }, (_, index) => ({ ...sampleScripts()[1], id: 1000 + index, name: `script-${String(index).padStart(3, '0')}`, approval: 'local' as const }));
    const invoke = fakeMemory({ kind: 'available' }, many);
    await showMemory();

    expect(screen.getByText('1–50 of 120')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    expect(await screen.findByText('51–100 of 120')).toBeInTheDocument();
    expect(calls(invoke, 'memory_scripts_list').at(-1)).toMatchObject({ offset: 50, limit: 50 });

    await fireEvent.click(screen.getByRole('button', { name: 'Page 3' }));

    expect(await screen.findByText('101–120 of 120')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Next' })).toBeDisabled();
  });
});

describe('script panel', () => {
  it('opens beside the table with the description, the code and the facts, and follows the row chosen', async () => {
    const invoke = fakeMemory();
    await showMemory();

    expect(screen.queryByRole('region', { name: 'open-prs' })).toBeNull();
    await fireEvent.click(rowButton('open-prs') as HTMLElement);

    const panel = await screen.findByRole('region', { name: 'open-prs' });
    expect(within(panel).getByText('Lists the open pull requests of this repository with their review state.')).toBeInTheDocument();
    expect(within(panel).getByText('Bash · github.com/useful/harness-desktop')).toBeInTheDocument();
    expect(await within(panel).findByText(/gh pr list --state open/)).toBeInTheDocument();
    expect(calls(invoke, 'memory_script_get')[0]).toEqual({ id: 12, project: PROJECT });
    for (const heading of ['Description', 'Code', 'Actions', 'Details']) {
      expect(within(panel).getByRole('heading', { level: 3, name: heading })).toBeInTheDocument();
    }
    expect(within(panel).getByText('14')).toBeInTheDocument();
    expect(within(panel).getByText('Succeeded')).toBeInTheDocument();
    expect(within(panel).getByText('Approved')).toBeInTheDocument();

    await fireEvent.click(rowButton('ci-status') as HTMLElement);

    const next = await screen.findByRole('region', { name: 'ci-status' });
    expect(await within(next).findByText(/subprocess.check_output/)).toBeInTheDocument();
    expect(within(next).getByText('Failed with exit code 1')).toBeInTheDocument();
  });

  it('browses with the arrow keys while it is open, and closes on Escape with the focus back on the row', async () => {
    fakeMemory();
    await showMemory();
    const first = rowButton('open-prs') as HTMLElement;
    first.focus();

    await fireEvent.keyDown(first, { key: 'Enter' });
    await screen.findByRole('region', { name: 'open-prs' });

    await fireEvent.keyDown(first, { key: 'ArrowDown' });

    expect(await screen.findByRole('region', { name: 'ci-status' })).toBeInTheDocument();
    expect(document.activeElement).toBe(rowButton('ci-status'));

    await fireEvent.keyDown(document.activeElement as HTMLElement, { key: 'Escape' });

    await waitFor(() => expect(screen.queryByRole('region', { name: 'ci-status' })).toBeNull());
    expect(document.activeElement).toBe(rowButton('ci-status'));
  });

  it('moves the focus only, with no panel, while none is open', async () => {
    fakeMemory();
    await showMemory();
    const first = rowButton('open-prs') as HTMLElement;
    first.focus();

    await fireEvent.keyDown(first, { key: 'ArrowDown' });

    expect(document.activeElement).toBe(rowButton('ci-status'));
    expect(screen.queryByRole('region', { name: 'ci-status' })).toBeNull();
  });

  it('closes from its own button', async () => {
    fakeMemory();
    await showMemory();
    await fireEvent.click(rowButton('open-prs') as HTMLElement);
    const panel = await screen.findByRole('region', { name: 'open-prs' });

    await fireEvent.click(within(panel).getByRole('button', { name: 'Close' }));

    await waitFor(() => expect(screen.queryByRole('region', { name: 'open-prs' })).toBeNull());
  });

  it('marks a script that was never run, or written elsewhere, with a dash and a warning', async () => {
    fakeMemory();
    await showMemory();

    await fireEvent.click(rowButton('disk-hogs') as HTMLElement);

    const panel = await screen.findByRole('region', { name: 'disk-hogs' });
    await waitFor(() => expect(within(panel).getByText('Never')).toBeInTheDocument());
    expect(within(panel).getAllByText('Needs review').length).toBeGreaterThan(0);
    expect(within(panel).queryByText('Succeeded')).toBeNull();
  });
});

/** Makes the read of one script answer with other values than the list gave for it. */
function overrideDetail(invoke: ReturnType<typeof fakeMemory>, patch: Partial<ScriptDetail>): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;
  invoke.mockImplementation((async (name: CommandName, args: never) => {
    const result = await real(name as never, args);

    return name === 'memory_script_get' ? { ...(result as ScriptDetail), ...patch } : result;
  }) as typeof ipc.invoke);
}

describe('script detail facts', () => {
  it('shows the hash of the script that was read, not the one the list gave', async () => {
    const invoke = fakeMemory();
    overrideDetail(invoke, { sha256: 'abcdef0123456789'.repeat(4) });
    await showMemory();
    await fireEvent.click(rowButton('open-prs') as HTMLElement);

    const panel = await screen.findByRole('region', { name: 'open-prs' });
    await waitFor(() => expect(within(panel).getByText('abcdef01')).toBeInTheDocument());
  });

  it('shows the approval the list gave while the script is still being read', async () => {
    const invoke = fakeMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    invoke.mockImplementation(((name: CommandName, args: never) => (name === 'memory_script_get' ? new Promise(() => {}) : real(name as never, args))) as typeof ipc.invoke);
    await showMemory();
    await fireEvent.click(rowButton('open-prs') as HTMLElement);

    const panel = await screen.findByRole('region', { name: 'open-prs' });
    expect(within(panel).getByText('Approved')).toBeInTheDocument();
    expect(within(panel).getByRole('status', { name: 'Loading the script' })).toBeInTheDocument();
  });

  it('gives a repository script that names no repository a label instead of an empty scope', async () => {
    const scripts = sampleScripts().map((script) => (script.name === 'open-prs' ? { ...script, repo: null } : script));
    fakeMemory({ kind: 'available' }, scripts);
    await showMemory();

    expect(within(rowButton('open-prs') as HTMLElement).getByText('A repository')).toBeInTheDocument();
    await fireEvent.click(rowButton('open-prs') as HTMLElement);
    const panel = await screen.findByRole('region', { name: 'open-prs' });
    expect(within(panel).getAllByText('A repository')).toHaveLength(1);
  });

  it('writes invisible characters out and warns above the code', async () => {
    const scripts = sampleScripts().map((script) => (script.name === 'open-prs' ? { ...script, body: 'echo ok\u202E rm -rf /\u200B\n' } : script));
    fakeMemory({ kind: 'available' }, scripts);
    await showMemory();
    await fireEvent.click(rowButton('open-prs') as HTMLElement);

    expect(await screen.findByText('This script contains 2 invisible characters, shown below in red. Read it carefully before it runs.')).toBeInTheDocument();
    const marks = Array.from(document.querySelectorAll('[data-hidden-char]')).map((node) => node.textContent);
    expect(marks).toEqual(['⟨U+202E⟩', '⟨U+200B⟩']);
    expect(document.body.textContent).not.toContain('\u202E');
  });

  it('shows no warning for an ordinary script', async () => {
    fakeMemory();
    await showMemory();
    await fireEvent.click(rowButton('open-prs') as HTMLElement);
    await screen.findByText(/gh pr list --state open/);

    expect(screen.queryByText(/invisible character/)).toBeNull();
  });
});

describe('deleting a script', () => {
  async function askToDelete(name: string): Promise<void> {
    await fireEvent.click(rowButton(name) as HTMLElement);
    await fireEvent.click(within(await screen.findByRole('region', { name })).getByRole('button', { name: 'Delete' }));
  }

  it('asks first, names where the script lives, then deletes it and refreshes the list', async () => {
    const invoke = fakeMemory();
    await showMemory();

    await askToDelete('open-prs');

    expect(await screen.findByText('Delete this script?')).toBeInTheDocument();
    expect(screen.getByText('This script is kept in the team database. Deleting it removes it for everyone.')).toBeInTheDocument();
    expect(calls(invoke, 'memory_script_delete')).toHaveLength(0);

    await fireEvent.click(await screen.findByRole('button', { name: 'Delete script' }));

    await waitFor(() => expect(rowButton('open-prs')).toBeNull());
    expect(calls(invoke, 'memory_script_delete')).toEqual([{ id: 12, project: PROJECT }]);
    expect(calls(invoke, 'memory_scripts_list').length).toBeGreaterThan(1);
    // The panel of the script that is gone closes.
    await waitFor(() => expect(screen.queryByRole('region', { name: 'open-prs' })).toBeNull());
  });

  it('keeps the script when the confirmation is cancelled, and does not mention the team for a local one', async () => {
    const invoke = fakeMemory();
    await showMemory();

    await askToDelete('ci-status');
    expect(await screen.findByText('Delete this script?')).toBeInTheDocument();
    expect(screen.queryByText(/kept in the team database/)).toBeNull();

    await fireEvent.click(screen.getByRole('button', { name: 'Keep it' }));

    expect(calls(invoke, 'memory_script_delete')).toHaveLength(0);
    expect(rowButton('ci-status')).not.toBeNull();
  });

  it('says so when the delete fails and keeps the row', async () => {
    const invoke = fakeMemory();
    await showMemory();
    const real = invoke.getMockImplementation() as typeof ipc.invoke;
    invoke.mockImplementation(((name: CommandName, args: never) =>
      name === 'memory_script_delete' ? Promise.reject(IpcFailure.from({ code: 'internal', message: 'database is locked' })) : real(name as never, args)) as typeof ipc.invoke);

    await askToDelete('disk-hogs');
    await fireEvent.click(await screen.findByRole('button', { name: 'Delete script' }));

    await waitFor(() => expect(toasts.items.at(-1)).toMatchObject({ title: 'Could not delete the script', description: 'database is locked' }));
    expect(rowButton('disk-hogs')).not.toBeNull();
  });
});

describe('when there is nothing to list', () => {
  it('offers the Ecosystem page when Harness Memory is not installed', async () => {
    fakeMemory({ kind: 'notInstalled' });
    nav.go('scripts');
    render(ScriptsView);

    expect(await screen.findByText('Harness Memory is not installed')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Open Ecosystem' }));

    expect(nav.current).toBe('ecosystem');
  });

  it('asks for an update when this Harness Memory is too old for the view', async () => {
    fakeMemory({ kind: 'outdated' });
    nav.go('scripts');
    render(ScriptsView);

    expect(await screen.findByText('Your memory needs an update')).toBeInTheDocument();
    expect(screen.getByText('A newer version of Harness Memory is needed for this view.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Open Ecosystem' })).toBeInTheDocument();
  });

  it('shows what went wrong and tries again on request', async () => {
    const invoke = fakeMemory({ kind: 'failed', message: 'it exited with code 1' });
    nav.go('scripts');
    render(ScriptsView);

    expect(await screen.findByText('Harness Memory did not answer. it exited with code 1')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Try again' }));

    await waitFor(() => expect(calls(invoke, 'memory_scripts_list').length).toBeGreaterThan(1));
  });

  it('explains that agents create scripts for repeated actions when none are saved yet', async () => {
    fakeMemory({ kind: 'available' }, []);
    nav.go('scripts');
    render(ScriptsView);

    expect(await screen.findByText('No scripts yet')).toBeInTheDocument();
    expect(screen.getByText(/Agents save a script when they repeat the same action/)).toBeInTheDocument();
    expect(screen.queryByRole('searchbox', { name: 'Search scripts' })).toBeNull();
  });
});
