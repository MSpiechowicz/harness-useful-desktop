import { ChatSession } from '$lib/chat/session.svelte';
import { t } from '$lib/i18n/i18n.svelte';
import type { SessionRoutineTag } from '$lib/ipc/bindings/SessionRoutineTag';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { buffered } from './agent-tabs';
import type { ToastsStore } from './toasts.svelte';

/**
 * Attaches to the live window of a routine run: the backend replays the run's conversation and then sends what happens
 * next. Returns the session with its events flowing, or null when the run cannot be attached to (it ended, or the
 * backend refused). `quiet` drops that failure without a notification.
 */
export async function openRunWindow(transport: Transport, notifications: ToastsStore, routine: SessionRoutineTag, quiet: boolean): Promise<ChatSession | null> {
  const feed = buffered();
  let session: ChatSession;

  try {
    const info = await transport.invoke('routine_run_attach', { runId: routine.runId, onEvent: feed.onEvent });
    session = new ChatSession(info, transport, notifications);
  } catch (error) {
    if (!quiet) {
      notifications.push({ title: t('routines.toast.attachFailed'), description: IpcFailure.from(error).message, tone: 'danger' });
    }

    return null;
  }

  session.routine = routine;
  feed.bind(session);

  return session;
}
