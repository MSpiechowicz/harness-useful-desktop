import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { nav } from '$lib/stores/nav.svelte';

import { openRunChat } from './routine-open';
import { routines } from './routines-data.svelte';

/**
 * Shows the run the backend asked for (a click on a "needs you" notification): its chat tab. The runs going on are read
 * again first when the event is faster than the list. A run that has ended meanwhile leaves the Routines view open on it.
 */
export async function openRequestedRun(runId: string): Promise<void> {
  if (!routines.running.has(runId)) {
    await routines.refresh();
  }

  const run = routines.running.get(runId);

  if (run) {
    await openRunChat(run);
  } else {
    nav.go('routines');
  }
}

/**
 * Opens a saved session of a routine whose run is still going on as that run's chat tab, so the user sees it live and
 * can answer it: resuming it would fail, the run holds the session. Returns false for any other session, to be resumed
 * as usual.
 */
export async function openLiveRoutineSession(summary: Pick<SessionSummary, 'routine'>): Promise<boolean> {
  const routine = summary.routine;

  if (!routine || !routines.running.has(routine.runId)) {
    return false;
  }

  const tab = await agentSessions.attachRoutine(routine, { quiet: true });

  if (tab) {
    nav.go('chat');
  }

  return tab !== null;
}
