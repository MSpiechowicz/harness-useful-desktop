import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { ApprovalMode } from '$lib/ipc/bindings/ApprovalMode';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { SessionRoutineTag } from '$lib/ipc/bindings/SessionRoutineTag';
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
