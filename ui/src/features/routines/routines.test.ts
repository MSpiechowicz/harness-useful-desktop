import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import { IpcFailure } from '$lib/ipc/transport';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { routines } from './routines-data.svelte';
import { calls, fakeRoutines, fakeRoutinesWhere, pick, registerRoutinesHooks, rowOf, showRoutines } from './routines-test-support';

registerRoutinesHooks();

const MORNING = 'Morning review';

describe('Routines view', () => {
  it('lists the routines in one table, each with its scope, schedule and status, and opens no panel by itself', async () => {
    fakeRoutines();
    await showRoutines();

    const grid = screen.getByRole('grid', { name: 'Routines' });
    expect(within(grid).queryByText('This project')).not.toBeInTheDocument();
    expect(within(grid).queryByRole('rowheader')).not.toBeInTheDocument();
    expect(within(rowOf(MORNING)).getByText('Global')).toBeInTheDocument();
    expect(within(rowOf('Run the tests')).getByText('demo')).toBeInTheDocument();

    expect(within(rowOf(MORNING)).getByText('Active')).toBeInTheDocument();
    expect(within(rowOf(MORNING)).getByText('Weekdays at 09:00')).toBeInTheDocument();
    expect(within(rowOf('Run the tests')).getByText('Running')).toBeInTheDocument();
    expect(within(rowOf('Run the tests')).getByText('Every 30 minutes')).toBeInTheDocument();
    expect(within(rowOf('Triage new issues')).getByText('Needs you')).toBeInTheDocument();
    expect(within(rowOf('Triage new issues')).getByText('0 8-18/2 * * 1-5')).toBeInTheDocument();
    expect(within(rowOf('Weekly changelog')).getByText('Pending approval')).toBeInTheDocument();
    expect(within(rowOf('Check the build on start')).getByText('Off')).toBeInTheDocument();
    expect(within(rowOf('Check the build on start')).getByText('When the app starts')).toBeInTheDocument();

    expect(screen.queryByRole('region', { name: MORNING })).not.toBeInTheDocument();
  });

  it('says what waits for the user in the header and how many routines there are once, in the row of the filters', async () => {
    fakeRoutines();
    await showRoutines();

    expect(routines.pending).toBe(2);
    expect(routines.attention).toBe(3);
    expect(screen.getByText('2 to approve · 1 need you')).toBeInTheDocument();
    expect(screen.getAllByText('8 routines')).toHaveLength(1);

    await pick('Status: All', /^Off/);

    expect(await screen.findByText('1 routine')).toBeInTheDocument();
    expect(screen.queryByText('8 routines')).not.toBeInTheDocument();
  });

  it('counts the routines in the menus of the filters, from the counts of the backend', async () => {
    fakeRoutines();
    await showRoutines();

    await fireEvent.click(screen.getByRole('button', { name: 'When: All' }));
    const when = await screen.findByRole('menu', { name: 'When' });
    expect(within(when).getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual([
      'All 8',
      'Every day at 2',
      'Weekdays at 1',
      'Every N minutes 1',
      'Every N hours 1',
      'Cron 2',
      'When the app starts 1',
    ]);
    await fireEvent.keyDown(when, { key: 'Escape' });

    await fireEvent.click(screen.getByRole('button', { name: 'Status: All' }));
    const status = await screen.findByRole('menu', { name: 'Status' });
    expect(within(status).getAllByRole('menuitemradio').map((item) => item.textContent?.trim())).toEqual(['All 8', 'Active 3', 'Running 2', 'Pending approval 2', 'Off 1', 'Paused 0', 'Invalid 0']);

    expect(screen.queryByRole('button', { name: /^Source/ })).not.toBeInTheDocument();
  });

  it('narrows the list by schedule and status together with the search, and says when nothing is left', async () => {
    fakeRoutines();
    await showRoutines();

    await pick('Status: All', /^Pending approval/);
    expect(screen.getByText('Weekly changelog')).toBeInTheDocument();
    expect(screen.queryByText(MORNING)).not.toBeInTheDocument();
    expect(viewState.routinesStatus).toBe('pending');

    await pick('When: All', /^When the app starts/);
    expect(await screen.findByText('No routines match these filters.')).toBeInTheDocument();

    await pick('When: When the app starts', /^Cron/);
    expect(screen.getByText('Weekly changelog')).toBeInTheDocument();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search routines' }), { target: { value: 'nothing like it' } });
    expect(await screen.findByText('No routine matches your search.')).toBeInTheDocument();

    await fireEvent.input(screen.getByRole('searchbox', { name: 'Search routines' }), { target: { value: 'changelog' } });
    expect(screen.getByText('Weekly changelog')).toBeInTheDocument();
  });

  it('clears a filter with the x in its button, and only that one', async () => {
    fakeRoutines();
    await showRoutines();

    await pick('When: All', /^Cron/);
    await pick('Status: All', /^Pending approval/);

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Status filter' }));

    expect(screen.getByRole('button', { name: 'Status: All' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'When: Cron' })).toBeInTheDocument();
    expect(viewState.routinesWhen).toBe('cron');
    expect(viewState.routinesStatus).toBe('all');
  });

  it('remembers the filters when the view is left and shown again', async () => {
    fakeRoutines();
    await showRoutines();

    await pick('When: All', /^Cron/);
    cleanup();
    await showRoutines();

    expect(screen.getByRole('button', { name: 'When: Cron' })).toBeInTheDocument();
  });
});

describe('Opening a routine', () => {
  it('shows when it runs, what it does and its latest runs', async () => {
    fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf(MORNING));

    const panel = await screen.findByRole('region', { name: MORNING });

    expect(within(panel).getByText(/Review the open pull requests/)).toBeInTheDocument();
    expect(within(panel).getByText('Weekdays at 09:00')).toBeInTheDocument();
    expect(within(panel).getByText('OMP')).toBeInTheDocument();
    expect(within(panel).getByText('Write')).toBeInTheDocument();
    expect(viewState.routineId).not.toBeNull();

    const runs = await within(panel).findByRole('list', { name: 'Latest runs' });
    expect(within(runs).getAllByRole('button')).toHaveLength(3);
    expect(within(panel).getByRole('button', { name: /^Show all \d+ runs$/ })).toBeInTheDocument();
  });

  it('names the whole folder of a project routine, not just its short name', async () => {
    fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Run the tests'));

    const details = within(await screen.findByRole('region', { name: 'Run the tests' })).getByText('Details').closest('section') as HTMLElement;

    expect(within(details).getByText('Project')).toBeInTheDocument();
    expect(within(details).getByText('/home/demo/Projects/demo')).toBeInTheDocument();
  });

  it('spells out hidden characters in the name and the description an agent wrote', async () => {
    const { state } = fakeRoutines();
    await showRoutines();

    state.agentAdds('Back‮up');
    await routines.refresh();

    const name = 'Back⟨U+202E⟩up';
    await fireEvent.click(rowOf(name));

    expect(await screen.findByRole('region', { name })).toBeInTheDocument();
  });

  it('offers to answer a run that waits, from the routine and from the badge in the header', async () => {
    fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Triage new issues'));

    const panel = await screen.findByRole('region', { name: 'Triage new issues' });

    expect(within(panel).getByText('A run is waiting for your answer')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Open chat' })).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Open session' })).toBeInTheDocument();
  });
});

describe('Switching a routine', () => {
  it('switches a routine off and on from its row', async () => {
    const { invoke } = fakeRoutines();
    await showRoutines();

    await fireEvent.click(within(rowOf(MORNING)).getByRole('switch', { name: `Turn off ${MORNING}` }));

    await waitFor(() => expect(within(rowOf(MORNING)).getByText('Off')).toBeInTheDocument());
    expect(calls(invoke, 'routine_save')).toHaveLength(1);
    expect(screen.queryByRole('region', { name: MORNING })).not.toBeInTheDocument();

    await fireEvent.click(within(rowOf(MORNING)).getByRole('switch', { name: `Turn on ${MORNING}` }));
    await waitFor(() => expect(within(rowOf(MORNING)).getByText('Active')).toBeInTheDocument());
  });

  it('keeps the switch of a routine that waits for approval off, since a save would approve it unseen', async () => {
    const { invoke } = fakeRoutines();
    await showRoutines();

    const toggle = within(rowOf('Weekly changelog')).getByRole('switch');
    expect(toggle).toBeDisabled();
    expect(toggle).toHaveAttribute('aria-checked', 'false');

    await fireEvent.click(rowOf('Weekly changelog'));
    const panel = await screen.findByRole('region', { name: 'Weekly changelog' });

    expect(within(panel).getByRole('switch')).toBeDisabled();
    expect(within(panel).getByRole('button', { name: 'Edit' })).toBeDisabled();
    expect(within(panel).getByRole('button', { name: 'Run now' })).toBeDisabled();
    expect(calls(invoke, 'routine_save')).toHaveLength(0);
  });

  it('says in a toast that a save of a routine that waits for approval was refused, with the routine left as it was', async () => {
    fakeRoutines();
    await showRoutines();
    const pending = routines.views.find((view) => view.status === 'pending') as NonNullable<(typeof routines.views)[number]>;

    expect(await routines.toggle(pending, true)).toBe(false);

    expect(toasts.items[0]).toMatchObject({ title: 'Could not switch the routine', description: 'this routine waits for your approval and cannot be saved' });
    expect(routines.find(pending.id)?.status).toBe('pending');
  });
});

describe('Reviewing a routine an agent wrote', () => {
  it('shows what changed against the approved version, and nothing that stayed', async () => {
    fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Release notes'));

    const panel = await screen.findByRole('region', { name: 'Release notes' });
    const review = within(panel).getByText('An agent changed this routine').closest('div[role="status"]') as HTMLElement;

    expect(within(review).getByRole('group', { name: 'Changes to When' })).toHaveTextContent('Weekdays at 16:45');
    expect(within(review).getByRole('group', { name: 'Changes to When' })).toHaveTextContent('Every day at 16:45');
    expect(within(review).getByRole('group', { name: 'Changes to Stop after' })).toHaveTextContent('45 min');
    expect(within(review).getByRole('figure', { name: 'Changes to Prompt' })).toHaveTextContent('Publish them as a draft release');
    expect(within(review).queryByRole('group', { name: 'Changes to Name' })).not.toBeInTheDocument();
    expect(within(review).getByText('Runs tools without asking')).toBeInTheDocument();
  });

  it('warns that a routine that runs tools without asking does so, before the user approves it', async () => {
    fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Weekly changelog'));

    const review = within(await screen.findByRole('region', { name: 'Weekly changelog' })).getByText('An agent added this routine').closest('div[role="status"]') as HTMLElement;

    expect(within(review).getByText('Runs tools without asking')).toBeInTheDocument();
    expect(within(review).getByText('Once approved, this routine runs tools without asking anyone.')).toBeInTheDocument();
    expect(within(review).getByRole('button', { name: 'Reject and remove' })).toBeInTheDocument();
  });

  it('runs the routine once the user approves exactly that version', async () => {
    const { invoke } = fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Weekly changelog'));

    await fireEvent.click(within(await screen.findByRole('region', { name: 'Weekly changelog' })).getByRole('button', { name: 'Approve' }));

    await waitFor(() => expect(within(rowOf('Weekly changelog')).getByText('Active')).toBeInTheDocument());
    await waitFor(() => expect(toasts.items.map((toast) => [toast.title, toast.description])).toContainEqual(['Weekly changelog approved', 'Schedule: 0 8 * * 1.']));
    expect(calls(invoke, 'routine_approve')).toEqual([{ id: expect.stringMatching(/^[0-9a-f]{32}$/), sha256: expect.stringMatching(/^[0-9a-f]{64}$/) }]);
    expect(routines.pending).toBe(1);
  });

  it('puts an edited routine back as it was approved, and removes one that was never approved, when the user rejects it', async () => {
    fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Release notes'));

    await fireEvent.click(within(await screen.findByRole('region', { name: 'Release notes' })).getByRole('button', { name: 'Reject and undo' }));
    await waitFor(() => expect(within(rowOf('Release notes')).getByText('Active')).toBeInTheDocument());
    expect(within(rowOf('Release notes')).getByText('Weekdays at 16:45')).toBeInTheDocument();

    await fireEvent.click(rowOf('Weekly changelog'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Weekly changelog' })).getByRole('button', { name: 'Reject and remove' }));
    await waitFor(() => expect(screen.queryByText('Weekly changelog')).not.toBeInTheDocument());
  });
});

describe('Running a routine', () => {
  it('starts a run now, shows it running, and lets the user stop it', async () => {
    const { invoke } = fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf(MORNING));

    await fireEvent.click(within(await screen.findByRole('region', { name: MORNING })).getByRole('button', { name: 'Run now' }));

    await waitFor(() => expect(within(rowOf(MORNING)).getByText('Running')).toBeInTheDocument());
    expect(calls(invoke, 'routine_run_now')).toEqual([{ id: expect.stringMatching(/^[0-9a-f]{32}$/) }]);
    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain(`${MORNING} is running.`));

    await fireEvent.click(await within(screen.getByRole('region', { name: MORNING })).findByRole('button', { name: 'Stop' }));

    await waitFor(() => expect(within(rowOf(MORNING)).getByText('Active')).toBeInTheDocument());
    expect(calls(invoke, 'routine_run_stop')).toHaveLength(1);
  });
});

describe('A routine that cannot run for now', () => {
  const MISSING = 'The project folder /work/app is missing.';

  it('shows a project routine whose folder is missing as paused, with the reason, and keeps Run now off', async () => {
    const { invoke } = fakeRoutinesWhere((view) => (view.name === MORNING ? { ...view, status: 'paused', canRunNow: true, problem: MISSING } : view));
    await showRoutines();

    expect(within(rowOf(MORNING)).getByText('Paused')).toBeInTheDocument();
    await fireEvent.click(rowOf(MORNING));
    const panel = await screen.findByRole('region', { name: MORNING });

    expect(within(panel).getByText('Paused until the folder it runs in is back.')).toBeInTheDocument();
    expect(within(panel).getByText(MISSING)).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Run now' })).toBeDisabled();
    expect(calls(invoke, 'routine_run_now')).toHaveLength(0);
  });
});

describe('A routine that waits for a free place', () => {
  const queued = (view: RoutineView): RoutineView => ({ ...view, queued: true });

  it('shows as waiting, and Run now is off and says it starts by itself', async () => {
    const { invoke } = fakeRoutinesWhere((view) => (view.name === MORNING ? queued(view) : view));
    await showRoutines();

    expect(within(rowOf(MORNING)).getByText('Waiting')).toBeInTheDocument();
    await fireEvent.click(rowOf(MORNING));
    const panel = await screen.findByRole('region', { name: MORNING });

    expect(within(panel).getByText(/Waiting for a free place to run/)).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Run now' })).toBeDisabled();
    expect(calls(invoke, 'routine_run_now')).toHaveLength(0);
  });

  it('says it is already waiting when asked to run again, not that it changed meanwhile', async () => {
    const { invoke } = fakeRoutinesWhere((view) => (view.name === MORNING ? queued(view) : view));
    await showRoutines();
    const view = routines.views.find((candidate) => candidate.name === MORNING) as RoutineView;

    expect(await routines.runNow(view)).toBe(false);

    expect(toasts.items.at(-1)).toMatchObject({ title: 'Already waiting for a free place', description: MORNING, tone: 'info' });
    expect(calls(invoke, 'routine_run_now')).toHaveLength(0);
  });
});

describe('Asking to run a routine that started waiting meanwhile', () => {
  const conflict = (message: string) => () => {
    throw IpcFailure.from({ code: 'conflict', message });
  };

  it('says it is already waiting, not that the routine changed, once the list shows it waits', async () => {
    let waiting = false;
    const { invoke } = fakeRoutinesWhere((view) => (view.name === MORNING && waiting ? { ...view, queued: true } : view), {
      routine_run_now: () => {
        waiting = true;
        conflict('the routine is already waiting for a free place')();
      },
    });
    await showRoutines();
    await fireEvent.click(rowOf(MORNING));

    await fireEvent.click(within(await screen.findByRole('region', { name: MORNING })).getByRole('button', { name: 'Run now' }));

    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('Already waiting for a free place'));
    expect(toasts.items.map((toast) => toast.title)).not.toContain('This routine changed in the meantime.');
    expect(calls(invoke, 'routine_run_now')).toHaveLength(1);
    await waitFor(() => expect(within(rowOf(MORNING)).getByText('Waiting')).toBeInTheDocument());
  });

  it('still says the routine changed for a conflict that is not about waiting', async () => {
    fakeRoutinesWhere((view) => view, { routine_run_now: conflict('the routine can no longer run') });
    await showRoutines();
    await fireEvent.click(rowOf(MORNING));

    await fireEvent.click(within(await screen.findByRole('region', { name: MORNING })).getByRole('button', { name: 'Run now' }));

    await waitFor(() => expect(toasts.items.map((toast) => toast.title)).toContain('This routine changed in the meantime.'));
    expect(toasts.items.map((toast) => toast.title)).not.toContain('Already waiting for a free place');
  });
});

describe('The tray hint', () => {
  it('asks to keep the app in the tray while routines are on and the setting is off, and turns it on', async () => {
    fakeRoutines();
    await showRoutines();

    expect(screen.getByText('Routines stop when the window closes')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Turn on' }));

    await waitFor(() => expect(screen.queryByText('Routines stop when the window closes')).not.toBeInTheDocument());
    expect(prefs.value?.keepRunningInTray).toBe(true);
    await prefs.update({ keepRunningInTray: false });
  });
});

describe('Coming back to the view', () => {
  it('opens on the routine that was open when the user left', async () => {
    fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf(MORNING));
    await screen.findByRole('region', { name: MORNING });

    cleanup();
    await showRoutines();

    expect(await screen.findByRole('region', { name: MORNING })).toBeInTheDocument();
  });

  it('opens the editor when the command palette asks for a new routine', async () => {
    fakeRoutines();
    routines.wantsNew = true;
    await showRoutines();

    expect(await screen.findByRole('dialog', { name: 'New routine' })).toBeInTheDocument();
    expect(routines.wantsNew).toBe(false);
  });
});

describe('A view with nothing to show', () => {
  it('invites the user to add the first routine, and says agents can write them too', async () => {
    fakeRoutines('empty');
    await showRoutines();

    expect(screen.getByText('No routines yet')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'New routine' }).length).toBeGreaterThan(0);
    expect(await screen.findByText('Agents can create routines too, through the Harness Desktop MCP server. They wait for your approval.')).toBeInTheDocument();
  });

  it('says why the routines cannot be read and offers to try again', async () => {
    fakeRoutines('failing');
    await showRoutines();

    expect(await screen.findByText('Could not read the routines')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('says when the folder is not read', async () => {
    fakeRoutines('problem');
    await showRoutines();

    expect(await screen.findByText('Routines are not available')).toBeInTheDocument();
    expect(screen.getByText(/writable by other users/)).toBeInTheDocument();
  });

  it('lists a file that breaks the format with an unknown scope, and keeps it from running', async () => {
    fakeRoutines('invalid');
    await showRoutines();

    const row = [...document.querySelectorAll<HTMLElement>('[data-row]')].find((candidate) => within(candidate).queryByText('Invalid'));

    expect(row).toBeDefined();
    expect(within(row as HTMLElement).getByText('Unknown')).toBeInTheDocument();
    expect(within(row as HTMLElement).getByRole('switch')).toBeDisabled();
  });
});
