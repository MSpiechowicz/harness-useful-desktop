import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { t } from '$lib/i18n/i18n.svelte';
import type { SessionRoutineTag } from '$lib/ipc/bindings/SessionRoutineTag';
import { IpcFailure } from '$lib/ipc/transport';
import { generatedNameCount } from '$lib/chat/naming';
import type { ChatSession } from '$lib/chat/session.svelte';
import type { ChatMemory } from './memory-chat';

/** Options a restart can change; everything else carries over from the session being replaced. */
export interface RestartOptions {
  approvalOverride?: ApprovalMode | null;
  trustOnce?: boolean;
}

/** A chat whose agent is still starting: shown as a tab at once, replaced by the session when the agent is ready. */
export interface StartingChat {
  key: string;
  harness: HarnessId;
  project: string;
}

/** A chat of the last run to reopen: its conversation, and the name it had. */
export interface RestoredChat {
  harness: HarnessId;
  project: string;
  sessionId: string;
  label: ChatSession['aiLabel'];
  userNamed: boolean;
  /** Set for a tab that was the live window of a routine run: it attaches to the run again while it goes on. */
  routine?: SessionRoutineTag | null;
  /** The chat's memory identity from the last run, so the reopened chat keeps its key and guards. */
  memory?: ChatMemory | null;
}

/**
 * Events can arrive before the command that opens a session returns: they wait here until the session exists, then go
 * to it in order.
 */
export function buffered(): { onEvent: (envelope: AgentEnvelope) => void; bind: (session: ChatSession) => void } {
  const pending: AgentEnvelope[] = [];
  let session: ChatSession | null = null;

  return {
    onEvent: (envelope) => (session ? session.ingest(envelope) : void pending.push(envelope)),
    bind: (bound) => {
      session = bound;

      for (const envelope of pending) {
        bound.ingest(envelope);
      }
    },
  };
}

/**
 * A placeholder name for a new chat: a random one of the names no open chat has, so a fresh chat is not always the
 * first of the list. Once every name is taken, the lowest free number (a name with "2", "3"...) follows.
 */
export function nextNumber(taken: Iterable<number>): number {
  const used = new Set(taken);
  const free = Array.from({ length: generatedNameCount() }, (_, index) => index + 1).filter((number) => !used.has(number));
  if (free.length > 0) {
    return free[Math.floor(Math.random() * free.length)];
  }

  let next = 1;
  while (used.has(next)) {
    next += 1;
  }

  return next;
}

/**
 * What a failed agent start says. An agent that never answered or spoke wrongly is most often outdated, so that is said first, in the user's language.
 * Other failures (a missing binary, a folder to chmod) name their own fix and stay as they are.
 */
export function startFailureMessage(error: unknown): string {
  const failure = IpcFailure.from(error);
  const outdated = failure.code === 'timeout' || failure.code === 'protocol';

  return outdated ? `${t('chat.toast.startFailedHint')} ${failure.message}` : failure.message;
}
