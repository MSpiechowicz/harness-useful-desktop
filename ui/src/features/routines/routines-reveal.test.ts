import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
import type { RoutineListing } from '$lib/ipc/bindings/RoutineListing';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';
import { nav } from '$lib/stores/nav.svelte';
import { projectScopes } from '$lib/stores/project-scopes.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { listed } from '../../../tests/doc-support';
import { viewKey } from './routine-format';
import { calls, fakeRoutinesListing, PROJECT, registerRoutinesHooks, rowOf, showRoutines } from './routines-test-support';
import RoutinesView from './RoutinesView.svelte';
import { routines } from './routines-data.svelte';

registerRoutinesHooks();

const RESEARCH = '/home/demo/Notes/research';
const MORNING = 'Morning review';
const grid = () => screen.getByRole('grid', { name: 'Routines' });

/** The listing with enough more routines to make three pages. */
function bulky(listing: RoutineListing): RoutineListing {
  const [first] = listing.routines;
  const more = Array.from({ length: 112 }, (_, index): RoutineView => ({ ...first, id: `b${String(index).padStart(31, '0')}`, routine: first.routine && { ...first.routine, name: `Bulk ${String(index).padStart(3, '0')}` }, name: `Bulk ${String(index).padStart(3, '0')}` }));

  return { ...listing, routines: [...listing.routines, ...more] };
}

/** The listing with 64 routines of the research project, the most it can hold. */
function crowded(listing: RoutineListing): RoutineListing {
  const [first] = listing.routines;
  const more = Array.from({ length: 64 }, (_, index): RoutineView => ({
    ...first,
    id: `d${String(index).padStart(31, '0')}`,
    name: `Research ${String(index).padStart(2, '0')}`,
    origin: 'local',
    root: RESEARCH,
    repoKey: `path:${RESEARCH}`,
    routine: first.routine && { ...first.routine, name: `Research ${String(index).padStart(2, '0')}`, scope: 'project', project: RESEARCH },
  }));

  return { ...listing, routines: [...listing.routines, ...more] };
}

/** Answers `project_scopes` with `scopes` once `ready` resolves, as a slow Harness Memory does. */
function scopesWhen(invoke: ReturnType<typeof fakeRoutinesListing>['invoke'], ready: Promise<void>, scopes?: ProjectScope[]): void {
  const real = invoke.getMockImplementation() as typeof ipc.invoke;

  invoke.mockImplementation(((name: CommandName, args: never) => {
    if (name !== 'project_scopes') {
      return real(name as never, args);
    }

    return ready.then(() => scopes ?? real(name as never, args));
  }) as typeof ipc.invoke);
}

/** Shows the view as soon as its routines are read, not waiting for the projects. */
async function showBeforeProjects(): Promise<void> {
  nav.go('routines');
  render(RoutinesView);
  await waitFor(() => expect(screen.queryByRole('status', { name: 'Loading routines' })).not.toBeInTheDocument());
}

async function openMenu(): Promise<HTMLElement> {
  await fireEvent.click(screen.getByRole('button', { name: 'More actions' }));

  return screen.findByRole('menuitem', { name: 'Delete all routines…' });
}

describe('A Scope that is gone', () => {
  it('counts as All, so the routines are listed instead of all hidden', async () => {
    viewState.routinesScope = 'repo:git:github.com/forgotten/project';
    fakeRoutinesListing((listing) => listing);
    await showRoutines();

    expect(screen.getByRole('button', { name: 'Scope: All' })).toBeInTheDocument();
    expect(listed(grid())).toContain(MORNING);
  });
});

describe('Delete all while the projects are still being read', () => {
  it('stays off until the open project is known, then names it', async () => {
    const { invoke } = fakeRoutinesListing((listing) => listing);
    let release = () => {};
    scopesWhen(invoke, new Promise<void>((resolve) => (release = resolve)));
    await showBeforeProjects();

    const item = await openMenu();
    expect(item).toBeDisabled();

    release();
    await waitFor(() => expect(item).toBeEnabled());
    await fireEvent.click(item);

    expect(await screen.findByText(/^Delete the \d+ routines? of Global and demo\?$/)).toBeInTheDocument();
  });

  it('stays off when the projects lack the open one, because the delete would still reach it', async () => {
    const { invoke } = fakeRoutinesListing((listing) => listing);
    scopesWhen(invoke, Promise.resolve(), [{ path: RESEARCH, name: 'research', root: RESEARCH, key: `path:${RESEARCH}` }]);
    await showRoutines();

    expect(await openMenu()).toBeDisabled();
    expect(calls(invoke, 'routines_delete_all')).toHaveLength(0);
  });

  it('deletes with the path of the open project', async () => {
    const { invoke } = fakeRoutinesListing((listing) => listing);
    await showRoutines();

    await fireEvent.click(await openMenu());
    await fireEvent.click(await screen.findByRole('button', { name: 'Delete all routines' }));

    await waitFor(() => expect(calls(invoke, 'routines_delete_all')).toEqual([{ project: PROJECT }]));
  });
});

describe('A routine that opens', () => {
  it('lets go of the search and the Scope that hid it, and is shown selected', async () => {
    fakeRoutinesListing((listing) => listing);
    await showRoutines();
    viewState.routinesScope = 'global';
    viewState.routinesQuery = 'zzz';
    await waitFor(() => expect(screen.queryByRole('grid', { name: 'Routines' })).not.toBeInTheDocument());

    const morning = routines.views.find((view) => view.routine?.name === MORNING);
    viewState.routineId = viewKey(morning as NonNullable<typeof morning>);

    await waitFor(() => expect(viewState.routinesScope).toBe('all'));
    expect(viewState.routinesQuery).toBe('');
    expect(rowOf(MORNING)).toHaveAttribute('aria-selected', 'true');
  });

  it('turns the table to the page it is on', async () => {
    fakeRoutinesListing(bulky);
    await showRoutines();
    expect(viewState.routinesPage).toBe(0);

    viewState.routineId = `b${String(111).padStart(31, '0')}`;

    await waitFor(() => expect(viewState.routinesPage).toBe(2));
    expect(rowOf('Bulk 111')).toHaveAttribute('aria-selected', 'true');
  });

  it('turns the table to the routine chosen in the run history', async () => {
    fakeRoutinesListing(bulky);
    await showRoutines();
    viewState.routinesSection = 'runs';
    viewState.routinesQuery = 'zzz';

    viewState.routineId = `b${String(60).padStart(31, '0')}`;
    viewState.routinesSection = 'routines';

    await waitFor(() => expect(viewState.routinesQuery).toBe(''));
    await waitFor(() => expect(viewState.routinesPage).toBe(1));
    expect(rowOf('Bulk 060')).toHaveAttribute('aria-selected', 'true');
  });
});

describe('Editing a routine of a scope that holds the most', () => {
  it('can be saved unmoved, whenever the projects were read', async () => {
    const { invoke } = fakeRoutinesListing(crowded);
    let release = () => {};
    // The project is a folder inside the repository, so its path is not the root the routines are stored under.
    scopesWhen(invoke, new Promise<void>((resolve) => (release = resolve)), [
      { path: PROJECT, name: 'demo', root: PROJECT, key: 'git:github.com/useful/harness-desktop' },
      { path: `${RESEARCH}/notes`, name: 'research', root: RESEARCH, key: `path:${RESEARCH}` },
    ]);
    await showBeforeProjects();
    viewState.routinesQuery = 'Research 00';
    await fireEvent.click(await screen.findByText('Research 00', { selector: '[data-row] td span' }));
    await fireEvent.click(within(await screen.findByRole('region', { name: 'Research 00' })).getByRole('button', { name: 'Edit' }));
    const dialog = await screen.findByRole('dialog', { name: 'Edit routine' });

    release();
    await waitFor(() => expect(projectScopes.data).not.toBeNull());

    expect(within(dialog).queryByText(/already holds/)).not.toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Save' })).toBeEnabled();
  });
});
