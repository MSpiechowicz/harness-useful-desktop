import { render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeAll, beforeEach, expect, vi } from 'vitest';

import type { RoutineListing } from '$lib/ipc/bindings/RoutineListing';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { routineHandlers } from '$lib/ipc/mock/routine-handlers';
import { MockRoutines, type RoutineScenario } from '$lib/ipc/mock/routine-state';
import { nav } from '$lib/stores/nav.svelte';
import { projectScopes } from '$lib/stores/project-scopes.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { calls, pick, spyOnInvoke } from '../../../tests/doc-support';
import { startMockApp } from '../../../tests/mock-app';
import { routines } from './routines-data.svelte';
import RoutinesView from './RoutinesView.svelte';

export const PROJECT = fixtures.recentProjects[0].path;

/**
 * A routines folder kept in memory that answers the routine commands like the real one (the mock backend's own), with the rest
 * of the app talking to the mock transport as usual.
 */
export function fakeRoutines(scenario: RoutineScenario = 'ok') {
  const state = new MockRoutines(() => scenario);

  return { state, invoke: spyOnInvoke(routineHandlers(state, (info) => info)) };
}

/** Like `fakeRoutines`, with the listing changed as `change` says (more routines, a routine of another project) and some commands answered as `answers` say. */
export function fakeRoutinesListing(change: (listing: RoutineListing) => RoutineListing, answers: object = {}, scenario: RoutineScenario = 'ok') {
  const state = new MockRoutines(() => scenario);
  const handlers = routineHandlers(state, (info) => info);
  const listed = handlers.routines_list;
  const changed = { ...handlers, ...answers, routines_list: async (args: Parameters<typeof listed>[0]): Promise<RoutineListing> => change(await listed(args)) };

  return { state, invoke: spyOnInvoke(changed) };
}

/** Like `fakeRoutines`, with each routine of the list changed as the backend would say it (a queued routine, a paused one), and some commands answered as `answers` say. */
export function fakeRoutinesWhere(change: (view: RoutineView) => RoutineView, answers: object = {}) {
  return fakeRoutinesListing((listing) => ({ ...listing, routines: listing.routines.map(change) }), answers);
}

export { calls, pick };

/** The table row of a routine, by its name. */
export function rowOf(name: string): HTMLElement {
  const cell = screen.getByText(name, { selector: '[data-row] td span' });

  return cell.closest<HTMLElement>('[data-row]') as HTMLElement;
}

/** Registers what every test of the Routines view needs: the mock backend with the project open, and a clean view before and after each test. */
export function registerRoutinesHooks(): void {
  beforeAll(() => startMockApp(PROJECT));

  beforeEach(() => {
    viewState.reset('routines');
    routines.reset();
    projectScopes.sync(null, null);
    // The window is in front, so the app says what the backend would otherwise put in a notification.
    routines.notices.focused = () => true;
  });

  afterEach(() => {
    nav.go('chat');
    toasts.items = [];
    vi.restoreAllMocks();
  });
}

/** Shows the Routines view once it has read the routines (or failed to) and the projects its Scope names. */
export async function showRoutines(): Promise<void> {
  nav.go('routines');
  render(RoutinesView);
  await waitFor(() => expect(screen.queryByRole('status', { name: 'Loading routines' })).not.toBeInTheDocument());
  await waitFor(() => expect(projectScopes.data).not.toBeNull());
}
