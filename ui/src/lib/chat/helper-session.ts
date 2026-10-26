import type { AgentAction } from '$lib/ipc/bindings/AgentAction';
import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentReply } from '$lib/ipc/bindings/AgentReply';
import type { AgentSessionInfo } from '$lib/ipc/bindings/AgentSessionInfo';
import type { Transport } from '$lib/ipc/transport';

import { reduceEnvelope } from './reducer';
import { initialChatState } from './state';
import type { ChatState } from './types';

/** A helper that has not answered by now is dropped. */
const TIMEOUT_MS = 45_000;

export interface HelperRequest {
  harness: AgentSessionInfo['harness'];
  project: string;
}

/** What a piece of work can do with its throwaway agent. */
export interface Helper {
  /** Runs `action` in the helper and returns its reply. */
  send: (action: AgentAction) => Promise<AgentReply>;
  /** The helper's conversation as far as its events have come. */
  state: () => ChatState;
  /** Resolves once `done` holds for the helper's state, false when time runs out or the helper exits first. */
  until: (done: (state: ChatState) => boolean) => Promise<boolean>;
}

/**
 * Runs `work` with a throwaway agent of `request.harness` in `request.project`: no tools to run unasked, no saved session
 * and never among the chats. It starts once the agent is ready and is always stopped. Null when the agent cannot start,
 * is never ready, or `work` fails.
 */
export async function withHelper<T>(transport: Transport, request: HelperRequest, work: (helper: Helper) => Promise<T>): Promise<T | null> {
  let started: AgentSessionInfo | null = null;
  const pending: AgentEnvelope[] = [];
  let state: ChatState | null = null;
  let wake: (() => void) | null = null;

  const onEvent = (envelope: AgentEnvelope): void => {
    if (state) {
      state = reduceEnvelope(state, envelope);
      wake?.();
    } else {
      pending.push(envelope);
    }
  };

  const until: Helper['until'] = (done) =>
    new Promise((resolve) => {
      const timer = setTimeout(() => resolve(false), TIMEOUT_MS);
      const check = (): void => {
        if (state && (done(state) || state.phase === 'exited')) {
          clearTimeout(timer);
          wake = null;
          resolve(done(state));
        }
      };

      wake = check;
      check();
    });

  try {
    started = await transport.invoke('agent_start', {
      request: { harness: request.harness, project: request.project, resume: null, approvalOverride: 'alwaysAsk', trustOnce: false, utility: true, memoryChat: null },
      onEvent,
    });

    const key = started.key;
    state = initialChatState(key);
    for (const envelope of pending) {
      state = reduceEnvelope(state, envelope);
    }

    if (!(await until((current) => current.readyCount > 0))) {
      return null;
    }

    return await work({
      send: (action) => transport.invoke('agent_send', { key, action }),
      state: () => state ?? initialChatState(key),
      until,
    });
  } catch {
    return null;
  } finally {
    if (started) {
      const key = started.key;

      void transport.invoke('agent_stop', { key }).catch(() => undefined);
    }
  }
}
