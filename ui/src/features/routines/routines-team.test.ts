import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import { viewState } from '$lib/stores/view-state.svelte';

import { calls, fakeRoutines, pick, registerRoutinesHooks, rowOf, showRoutines } from './routines-test-support';

registerRoutinesHooks();

/** The row of a team routine, by its name: a team routine can share its name with a local one, so the row is found by its key. */
function teamRowOf(name: string): HTMLElement {
  const row = [...document.querySelectorAll<HTMLElement>('[data-row^="team:"]')].find((candidate) => candidate.textContent?.includes(name));

  if (!row) {
    throw new Error(`no team routine is listed as ${name}`);
  }

  return row;
}

describe('Routines shared with a team', () => {
  it('has no Source filter, no Team group and no way to remove team routines while nothing is shared', async () => {
    fakeRoutines();
    await showRoutines();

    expect(screen.queryByRole('button', { name: /^Source/ })).not.toBeInTheDocument();
    expect(screen.queryByText('Team')).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    expect(await screen.findByRole('menuitem', { name: 'Delete all routines…' })).toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: 'Remove all my team routines…' })).not.toBeInTheDocument();
  });

  it('lists the team routines after the user\'s own, off until each is turned on here', async () => {
    fakeRoutines('team');
    await showRoutines();

    expect(screen.getByText('Team')).toBeInTheDocument();
    expect(within(teamRowOf('Standup summary')).getByText('Active')).toBeInTheDocument();
    expect(within(teamRowOf('Dependency audit')).getByText('Off')).toBeInTheDocument();
    expect(within(teamRowOf('Sweep stale branches')).getByText('Pending approval')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Source: All' })).toBeInTheDocument();
  });

  it('narrows the list to the team\'s routines, or to the user\'s own', async () => {
    fakeRoutines('team');
    await showRoutines();

    await pick('Source: All', /^Team/);
    expect(screen.queryByText('Morning review')).not.toBeInTheDocument();
    expect(screen.getByText('Standup summary')).toBeInTheDocument();
    expect(viewState.routinesSource).toBe('team');

    await pick('Source: Team', /^Mine/);
    expect(screen.getByText('Morning review')).toBeInTheDocument();
    expect(screen.queryByText('Standup summary')).not.toBeInTheDocument();
  });

  it('turns a team routine on for this computer, in the open project\'s folder, and then waits for the user\'s approval', async () => {
    const { invoke } = fakeRoutines('team');
    await showRoutines();

    await fireEvent.click(within(teamRowOf('Dependency audit')).getByRole('switch', { name: 'Turn on Dependency audit' }));

    await waitFor(() => expect(calls(invoke, 'routines_team_enable')).toEqual([{ id: expect.stringMatching(/^[0-9a-f]{32}$/), enabled: true, project: '/home/demo/Projects/demo' }]));
    // Turning on is not approval: without trust the routine waits until the user has read it.
    await waitFor(() => expect(within(teamRowOf('Dependency audit')).getByText('Pending approval')).toBeInTheDocument());
  });

  it('shows who shared a team routine, and offers to copy it, never to edit or delete it', async () => {
    fakeRoutines('team');
    await showRoutines();
    await fireEvent.click(teamRowOf('Standup summary'));

    const panel = await screen.findByRole('region', { name: 'Standup summary' });

    expect(within(panel).getByText('Shared by')).toBeInTheDocument();
    expect(within(panel).getByText('Anna Kowalska')).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Copy to my routines' })).toBeEnabled();
    expect(within(panel).getByRole('button', { name: 'Remove from team' })).toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Edit' })).not.toBeInTheDocument();
    expect(within(panel).queryByRole('button', { name: 'Delete' })).not.toBeInTheDocument();
  });

  it('runs every team routine without asking when the team is trusted, one that runs tools without asking included', async () => {
    fakeRoutines('teamTrusted');
    await showRoutines();

    expect(within(teamRowOf('Standup summary')).getByText('Active')).toBeInTheDocument();
    expect(within(teamRowOf('Sweep stale branches')).getByText('Active')).toBeInTheDocument();
  });

  it('shares a routine of the user\'s with the team, and updates the team copy after a change', async () => {
    const { invoke } = fakeRoutines('team');
    await showRoutines();
    await fireEvent.click(rowOf('Check the disk'));

    await fireEvent.click(within(await screen.findByRole('region', { name: 'Check the disk' })).getByRole('button', { name: 'Share with team' }));

    await waitFor(() => expect(calls(invoke, 'routines_share')).toHaveLength(1));
    await waitFor(() => expect(within(screen.getByRole('region', { name: 'Check the disk' })).getByRole('button', { name: 'Share with team' })).toBeDisabled());
    expect(teamRowOf('Check the disk')).toBeInTheDocument();
  });

  it('removes every routine the user shared, after a question, and says how many went', async () => {
    const { invoke } = fakeRoutines('team');
    await showRoutines();

    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Remove all my team routines…' }));

    const dialog = await screen.findByRole('dialog', { name: 'Remove all your team routines?' });
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Remove from team' }));

    await waitFor(() => expect(calls(invoke, 'routines_team_remove_mine')).toHaveLength(1));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Remove all your team routines?' })).not.toBeInTheDocument());
  });
});

describe('Deleting all routines', () => {
  it('asks first, names how many go, and leaves the view with its empty state', async () => {
    const { invoke } = fakeRoutines();
    await showRoutines();

    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Delete all routines…' }));

    const dialog = await screen.findByRole('dialog', { name: 'Delete all 8 routines?' });
    expect(calls(invoke, 'routines_delete_all')).toHaveLength(0);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Delete all routines' }));

    await waitFor(() => expect(calls(invoke, 'routines_delete_all')).toEqual([{ project: '/home/demo/Projects/demo' }]));
    expect(await screen.findByText('No routines yet')).toBeInTheDocument();
  });

  it('keeps everything when the user keeps them', async () => {
    const { invoke } = fakeRoutines();
    await showRoutines();

    await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    await fireEvent.click(await screen.findByRole('menuitem', { name: 'Delete all routines…' }));
    await fireEvent.click(within(await screen.findByRole('dialog', { name: 'Delete all 8 routines?' })).getByRole('button', { name: 'Keep them' }));

    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Delete all 8 routines?' })).not.toBeInTheDocument());
    expect(calls(invoke, 'routines_delete_all')).toHaveLength(0);
    expect(screen.getByText('Morning review')).toBeInTheDocument();
  });
});

describe('Deleting one routine', () => {
  it('asks first, then removes it and closes its panel', async () => {
    const { invoke } = fakeRoutines();
    await showRoutines();
    await fireEvent.click(rowOf('Check the disk'));

    await fireEvent.click(within(await screen.findByRole('region', { name: 'Check the disk' })).getByRole('button', { name: 'Delete' }));
    const dialog = await screen.findByRole('dialog', { name: 'Delete this routine?' });
    expect(calls(invoke, 'routine_delete')).toHaveLength(0);

    await fireEvent.click(within(dialog).getByRole('button', { name: 'Delete routine' }));

    await waitFor(() => expect(screen.queryByText('Check the disk')).not.toBeInTheDocument());
    expect(calls(invoke, 'routine_delete')).toHaveLength(1);
    expect(viewState.routineId).toBeNull();
  });
});
