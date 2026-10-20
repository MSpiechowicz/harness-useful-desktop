import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { RoutineListing } from '$lib/ipc/bindings/RoutineListing';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import { viewState } from '$lib/stores/view-state.svelte';

import { listed } from '../../../tests/doc-support';
import { calls, fakeRoutinesListing, pick, registerRoutinesHooks, rowOf, showRoutines } from './routines-test-support';

registerRoutinesHooks();

const RESEARCH = '/home/demo/Notes/research';
const MORNING = 'Morning review';

const grid = () => screen.getByRole('grid', { name: 'Routines' });

/** The listing with 'Check the build on start' moved to the research project, a folder the open project is not. */
function inResearch(listing: RoutineListing): RoutineListing {
  const moved = (view: RoutineView): RoutineView => (view.routine?.name === 'Check the build on start' ? { ...view, root: RESEARCH, repoKey: `path:${RESEARCH}`, routine: { ...view.routine, project: RESEARCH } } : view);

  return { ...listing, routines: listing.routines.map(moved) };
}

/** The listing with enough more routines to make three pages. */
function bulky(listing: RoutineListing): RoutineListing {
  const [first] = listing.routines;
  const more = Array.from({ length: 112 }, (_, index): RoutineView => ({ ...first, id: `b${String(index).padStart(31, '0')}`, routine: first.routine && { ...first.routine, name: `Bulk ${String(index).padStart(3, '0')}` }, name: `Bulk ${String(index).padStart(3, '0')}` }));

  return { ...listing, routines: [...listing.routines, ...more] };
}

/** The row of a remote routine, by its name. */
function teamRowOf(name: string): HTMLElement {
  const row = [...document.querySelectorAll<HTMLElement>('[data-row^="team:"]')].find((candidate) => candidate.textContent?.includes(name));

  if (!row) {
    throw new Error(`no remote routine is listed as ${name}`);
  }

  return row;
}

/** The entries of the Scope filter, as the menu shows them. */
async function scopeChoices(): Promise<string[]> {
  await fireEvent.click(screen.getByRole('button', { name: /^Scope:/ }));
  const menu = await screen.findByRole('menu', { name: 'Scope' });
  const choices = within(menu).getAllByRole('menuitemradio').map((item) => item.textContent?.replace(/\s+/g, ' ').trim() ?? '');

  await fireEvent.keyDown(menu, { key: 'Escape' });

  return choices;
}

describe('the Scope of the routines table', () => {
  it('lists the routines of every project with the global ones and a remote routine of a repository not on this computer, one table without group headings', async () => {
    const { invoke } = fakeRoutinesListing(inResearch, {}, 'team');
    await showRoutines();

    expect(calls(invoke, 'routines_list')[0]).toMatchObject({ all: true });
    expect(within(grid()).queryByText('Files to fix')).not.toBeInTheDocument();
    expect(screen.queryByText(/of 64/)).not.toBeInTheDocument();
    expect(within(rowOf(MORNING)).getByText('Global')).toBeInTheDocument();
    expect(within(rowOf('Run the tests')).getByText('demo')).toBeInTheDocument();
    expect(within(rowOf('Check the build on start')).getByText('research')).toBeInTheDocument();
    expect(within(teamRowOf('Dependency audit')).getByText('demo')).toBeInTheDocument();
    expect(within(teamRowOf('Deploy the preview site')).getByText('acme/website')).toBeInTheDocument();
    expect(within(teamRowOf('Deploy the preview site')).getByText('Remote')).toBeInTheDocument();
  });

  it('puts the Scope filter first in the row of the filters, offers every repository with its count and Remote only for the ones this computer lacks', async () => {
    fakeRoutinesListing(inResearch, {}, 'team');
    await showRoutines();

    const scope = screen.getByRole('button', { name: 'Scope: All' });

    expect(scope.compareDocumentPosition(screen.getByRole('button', { name: 'When: All' })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(await scopeChoices()).toEqual(['All 14', 'demo useful/harness-desktop 4', 'legacy-api 0', 'research 1', 'acme/website Remote only 1', 'Global 8']);
  });

  it('narrows the table to the routines of a project, and back to all of them', async () => {
    fakeRoutinesListing(inResearch, {}, 'team');
    await showRoutines();

    await pick('Scope: All', /^research/);

    expect(viewState.routinesScope).toMatch(/^repo:/);
    expect(listed(grid())).toEqual(['Check the build on start']);
    expect(await screen.findByText('1 routine')).toBeInTheDocument();

    await pick('Scope: research', /^demo/);

    expect(listed(grid())).toEqual(['Release notes', 'Run the tests', 'Triage new issues', 'Dependency audit']);

    await fireEvent.click(screen.getByRole('button', { name: 'Clear the Scope filter' }));

    expect(viewState.routinesScope).toBe('all');
    expect(listed(grid())).toHaveLength(14);
  });

  it('shows the routines of a repository only the team has under Remote only, and the global ones under Global', async () => {
    fakeRoutinesListing(inResearch, {}, 'team');
    await showRoutines();

    await pick('Scope: All', /^acme\/website/);
    expect(listed(grid())).toEqual(['Deploy the preview site']);

    await pick('Scope: acme/website', /^Global/);
    expect(listed(grid())).toEqual(['Check the disk', 'Morning review', 'Keep commits small (evening)', 'Look for hogs', 'Weekly changelog', 'Standup summary', 'Sweep stale branches', 'Keep commits small']);
    expect(screen.queryByText('Run the tests')).not.toBeInTheDocument();
    expect(within(grid()).getAllByText('Global').length).toBeGreaterThan(0);
  });

  it('says so for a project without routines, and keeps the choice', async () => {
    fakeRoutinesListing(inResearch);
    await showRoutines();

    await pick('Scope: All', /^legacy-api/);

    expect(await screen.findByText('No routines match these filters.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Scope: legacy-api' })).toBeInTheDocument();
  });

  it('pages the table 50 at a time on the client, and starts again from the first page when the table is narrowed', async () => {
    fakeRoutinesListing(bulky);
    await showRoutines();

    expect(listed(grid())).toHaveLength(50);
    expect(screen.getByText('1–50 of 120')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Next' }));

    expect(viewState.routinesPage).toBe(1);
    expect(screen.getByText('51–100 of 120')).toBeInTheDocument();

    await fireEvent.click(screen.getByRole('button', { name: 'Page 3' }));

    expect(listed(grid())).toHaveLength(20);
    expect(screen.getByText('101–120 of 120')).toBeInTheDocument();

    await pick('Status: All', /^Pending/);

    expect(viewState.routinesPage).toBe(0);
    expect(listed(grid())).toEqual(['Weekly changelog', 'Release notes']);
    expect(screen.queryByRole('button', { name: 'Page 3' })).not.toBeInTheDocument();
  });

  it('keeps the page the user was on when the view is left and opened again', async () => {
    fakeRoutinesListing(bulky);
    viewState.routinesPage = 1;
    await showRoutines();

    expect(screen.getByText('51–100 of 120')).toBeInTheDocument();
  });
});

describe('the Applies to field of the routine editor', () => {
  async function openEditor(): Promise<HTMLElement> {
    await fireEvent.click(screen.getAllByRole('button', { name: 'New routine' })[0]);

    return screen.findByRole('dialog', { name: 'New routine' });
  }

  const appliesTo = (dialog: HTMLElement) => within(dialog).getByRole('button', { name: 'Applies to' });

  it('starts where the Scope filter looks: in the project it shows, in Global for Global, else in Global', async () => {
    fakeRoutinesListing(inResearch);
    await showRoutines();

    let dialog = await openEditor();
    expect(appliesTo(dialog)).toHaveTextContent('Global');
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'New routine' })).not.toBeInTheDocument());

    await pick('Scope: All', /^research/);
    dialog = await openEditor();
    expect(appliesTo(dialog)).toHaveTextContent('research');
    expect(within(dialog).getByText('Runs in the folder of research.')).toBeInTheDocument();
  });

  it('saves a routine in a recent project that is not the open one, with that project\'s folder', async () => {
    const { invoke } = fakeRoutinesListing(inResearch);
    await showRoutines();
    const dialog = await openEditor();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'Tidy the notes' } });
    await fireEvent.input(within(dialog).getByLabelText('Prompt'), { target: { value: 'Tidy up.' } });
    await fireEvent.click(appliesTo(dialog));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: /^research/ }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Add routine' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { name: 'Tidy the notes', scope: 'project', project: RESEARCH } });
  });

  it('counts the routines of the chosen project and not the open one when it says the scope is full', async () => {
    const crowded = (listing: RoutineListing): RoutineListing => {
      const [first] = inResearch(listing).routines;
      const more = Array.from({ length: 64 }, (_, index): RoutineView => ({ ...first, id: `d${String(index).padStart(31, '0')}`, origin: 'local', root: RESEARCH, routine: first.routine && { ...first.routine, scope: 'project', project: RESEARCH } }));

      return { ...listing, routines: [...listing.routines, ...more] };
    };
    fakeRoutinesListing(crowded);
    await showRoutines();
    const dialog = await openEditor();

    await fireEvent.input(within(dialog).getByLabelText('Name'), { target: { value: 'One too many' } });
    await fireEvent.input(within(dialog).getByLabelText('Prompt'), { target: { value: 'Tidy up.' } });
    expect(within(dialog).getByRole('button', { name: 'Add routine' })).toBeEnabled();

    await fireEvent.click(appliesTo(dialog));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: /^research/ }));

    expect(within(dialog).getByText('This scope already holds 64 routines, the most it can.')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Add routine' })).toBeDisabled();
  });

  it('names its own project in the panel and in the question before it is deleted, never This project', async () => {
    fakeRoutinesListing(inResearch);
    await showRoutines();

    await fireEvent.click(rowOf('Check the build on start'));
    const panel = await screen.findByRole('region', { name: 'Check the build on start' });

    expect(within(panel).getByText(/· research$/)).toBeInTheDocument();
    expect(within(panel).queryByText(/This project/)).not.toBeInTheDocument();

    await fireEvent.click(within(panel).getByRole('button', { name: 'Delete' }));

    expect(within(await screen.findByRole('dialog', { name: 'Delete this routine?' })).getByText(/· research$/)).toBeInTheDocument();
  });

  it('opens a routine of another project on that project, and moves it to Global when asked', async () => {
    const { invoke } = fakeRoutinesListing(inResearch);
    await showRoutines();
    await fireEvent.click(rowOf('Check the build on start'));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Check the build on start' })).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit routine' });

    expect(appliesTo(dialog)).toHaveTextContent('research');

    await fireEvent.click(appliesTo(dialog));
    await fireEvent.click(await screen.findByRole('menuitemradio', { name: /^Global/ }));
    await fireEvent.click(within(dialog).getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(calls(invoke, 'routine_save')).toHaveLength(1));
    expect(calls(invoke, 'routine_save')[0]).toMatchObject({ draft: { scope: 'global', project: null } });
  });
});

describe('the remote routines of other repositories', () => {
  it('lock their switch and their copy while no project on this computer belongs to the repository, and never turn them on in the open project', async () => {
    const { invoke } = fakeRoutinesListing(inResearch, {}, 'team');
    await showRoutines();

    const toggle = within(teamRowOf('Deploy the preview site')).getByRole('switch', { name: 'Turn on Deploy the preview site' });
    expect(toggle).toBeDisabled();

    await fireEvent.click(teamRowOf('Deploy the preview site'));
    const panel = await screen.findByRole('region', { name: 'Deploy the preview site' });

    expect(within(panel).getByRole('button', { name: 'Copy to my routines' })).toBeDisabled();
    expect(within(panel).getByRole('switch')).toBeDisabled();
    expect(calls(invoke, 'routines_team_enable')).toHaveLength(0);
  });

  it('turn on and copy in the folder of the project that has their repository, which need not be the open one', async () => {
    const elsewhere = (listing: RoutineListing): RoutineListing => ({
      ...listing,
      routines: listing.routines.map((view) => (view.origin === 'team' && view.teamRepo !== null ? { ...view, teamRepo: `path:${RESEARCH}`, repoKey: `path:${RESEARCH}`, root: null } : view)),
    });
    const { invoke } = fakeRoutinesListing(elsewhere, {}, 'team');
    await showRoutines();

    expect(within(teamRowOf('Dependency audit')).getByText('research')).toBeInTheDocument();

    await fireEvent.click(within(teamRowOf('Dependency audit')).getByRole('switch', { name: 'Turn on Dependency audit' }));

    await waitFor(() => expect(calls(invoke, 'routines_team_enable')).toEqual([{ id: expect.stringMatching(/^[0-9a-f]{32}$/), enabled: true, project: RESEARCH }]));
  });
});
