import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { viewState } from '$lib/stores/view-state.svelte';

import { RUNS_PAGE_SIZE } from './routines-data.svelte';
import { calls, fakeRoutines, PROJECT, pick, registerRoutinesHooks, rowOf, showRoutines } from './routines-test-support';

registerRoutinesHooks();

/** Shows the run history instead of the list of routines. */
async function showHistory(): Promise<HTMLElement> {
  await showRoutines();
  await pick('Show: Routines', /^Run history/);

  return screen.findByRole('grid', { name: 'Run history' });
}

describe('Run history', () => {
  it('lists the runs newest first, a page of fifty, with how each went', async () => {
    const { invoke } = fakeRoutines();
    const grid = await showHistory();

    expect(RUNS_PAGE_SIZE).toBe(50);
    expect(within(grid).getAllByRole('row').length - 1).toBe(50);
    expect(calls(invoke, 'routine_runs').at(-1)).toMatchObject({ query: { routineId: null, status: null, offset: 0, limit: 50 } });
    expect(screen.getByText('122 runs')).toBeInTheDocument();
    expect(screen.getByText('1–50 of 122')).toBeInTheDocument();

    // The two runs going on lead.
    const first = within(grid).getAllByRole('row')[1];
    expect(within(first).getByText('Running')).toBeInTheDocument();
  });

  it('turns the pages with the server, never holding more than one page', async () => {
    const { invoke } = fakeRoutines();
    await showHistory();

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    await waitFor(() => expect(screen.getByText('51–100 of 122')).toBeInTheDocument());
    expect(calls(invoke, 'routine_runs').at(-1)).toMatchObject({ query: { offset: 50, limit: 50 } });

    await fireEvent.click(screen.getByRole('button', { name: 'Page 3' }));

    await waitFor(() => expect(screen.getByText('101–122 of 122')).toBeInTheDocument());
    expect(viewState.runsPage).toBe(2);
  });

  it('narrows the runs by status on the server and starts again from the first page', async () => {
    const { invoke } = fakeRoutines();
    await showHistory();
    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    await waitFor(() => expect(viewState.runsPage).toBe(1));

    await pick('Status: All', /^Failed/);

    await waitFor(() => expect(calls(invoke, 'routine_runs').at(-1)).toMatchObject({ query: { status: 'failed', offset: 0 } }));
    await waitFor(() => expect(screen.getByRole('button', { name: 'Status: Failed' })).toBeInTheDocument());
    const grid = screen.getByRole('grid', { name: 'Run history' });
    await waitFor(() => expect(within(grid).getAllByText('Failed').length).toBe(within(grid).getAllByRole('row').length - 1));
    expect(viewState.runsPage).toBe(0);
  });

  it('shows how a run went in its panel, with the reply or the output and the errors', async () => {
    fakeRoutines();
    const grid = await showHistory();
    const failed = within(grid).getAllByRole('row').find((row) => within(row).queryByText('Failed') && within(row).queryByText('Check the disk'));

    expect(failed).toBeDefined();
    await fireEvent.click(failed as HTMLElement);

    const panel = await screen.findByRole('region', { name: 'Check the disk' });
    expect(within(panel).getByText('The run ended with an error.')).toBeInTheDocument();
    expect(within(panel).getByText('Exit code')).toBeInTheDocument();
    expect(within(panel).getByText('Errors')).toBeInTheDocument();
    expect(within(panel).getByText(/the command did not finish cleanly/)).toBeInTheDocument();
  });

  it('opens the whole history of one routine from its panel, and its routine from the history', async () => {
    fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Morning review'));

    await fireEvent.click(await screen.findByRole('button', { name: /^Show all \d+ runs$/ }));

    await screen.findByRole('grid', { name: 'Run history' });
    expect(screen.getByRole('button', { name: /^Routine: Morning review/ })).toBeInTheDocument();
    expect(viewState.runsRoutine).not.toBeNull();

    const grid = screen.getByRole('grid', { name: 'Run history' });
    await fireEvent.click(within(grid).getAllByRole('row')[1]);
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Morning review' })).getByRole('button', { name: 'Show routine' }));

    expect(await screen.findByRole('region', { name: 'Morning review' })).toBeInTheDocument();
    expect(viewState.routinesSection).toBe('routines');
  });

  it('says that nothing has run yet, instead of showing an empty table', async () => {
    const { state } = fakeRoutines();
    state.deleteAll(PROJECT);
    state.agentAdds('Brand new');
    await showRoutines();

    await pick('Show: Routines', /^Run history/);

    expect(await screen.findByText('Nothing has run yet')).toBeInTheDocument();
    expect(screen.queryByRole('grid', { name: 'Run history' })).not.toBeInTheDocument();
  });
});
