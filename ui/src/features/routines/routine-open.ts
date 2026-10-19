import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { nav } from '$lib/stores/nav.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

/** What opening a run needs to know of it: the run, its routine, and the saved session it left, if any. */
export type OpenableRun = Pick<RoutineRun, 'id' | 'routineId' | 'routineName' | 'session' | 'project'>;

/**
 * Opens the chat session of a run in Sessions, in the folder it was kept in: the project's, or the routines' own for a global
 * routine. Sessions waits for that folder and shows the session or says it is missing, never another one.
 */
export function openRunSession(run: Pick<RoutineRun, 'session' | 'project'>): void {
  if (run.session) {
    viewState.sessionsFolder = run.project === null ? 'routines' : 'project';
    viewState.sessionId = run.session.id;
    viewState.sessionAsked = true;
  }

  nav.go('sessions');
}

/** Opens a routine in the Routines view, from a chip or a toast. */
export function showRoutine(routineId: string): void {
  viewState.routinesSection = 'routines';
  viewState.routineId = routineId;
  nav.go('routines');
}

/**
 * Opens a run that is going on as a chat tab and shows it, to answer its question or watch it work. A run that ended in
 * the meantime opens its saved session in Sessions instead, when it left one.
 */
export async function openRunChat(run: OpenableRun): Promise<void> {
  const tab = await agentSessions.attachRoutine({ runId: run.id, routineId: run.routineId, name: run.routineName }, { quiet: run.session !== null });

  if (tab) {
    nav.go('chat');
  } else if (run.session) {
    openRunSession(run);
  }
}
