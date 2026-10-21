import type { AgentEnvelope } from '$lib/ipc/bindings/AgentEnvelope';
import type { AgentSessionInfo } from '$lib/ipc/bindings/AgentSessionInfo';
import type { Transport } from '$lib/ipc/transport';

import { namingPrompt, parseLabel, type ChatLabel } from './naming';
import { reduceEnvelope } from './reducer';
import { initialChatState } from './state';
import type { ChatState } from './types';

/** A naming helper that has not answered by now is dropped; the chat keeps the name from its first message. */
const TIMEOUT_MS = 45_000;

/** The text of the helper's last assistant message. */
function replyText(state: ChatState): string {
  for (let index = state.items.length - 1; index >= 0; index -= 1) {
    const item = state.items[index];
    if (item.kind === 'message' && item.role === 'assistant') {
      return item.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n');
    }
  }

  return '';
}

export interface AiNameRequest {
  harness: AgentSessionInfo['harness'];
  project: string;
  /** The chat's model, so the helper costs what the chat does per token. */
  model: { provider: string; id: string } | null;
  prompt: string;
}

/**
 * Asks a throwaway agent of the chat's own harness to name the chat: one short request, no tools, no thinking, no
 * saved session, on the chat's model. The helper never appears among the chats and is always stopped. Null when it
 * fails, times out or answers with something that is not a label.
 */
export async function requestAiLabel(transport: Transport, request: AiNameRequest): Promise<ChatLabel | null> {
  let helper: AgentSessionInfo | null = null;
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

  /** Resolves once `done` holds for the helper's state, or when time runs out. */
  const until = (done: (state: ChatState) => boolean): Promise<boolean> =>
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
    helper = await transport.invoke('agent_start', {
      request: { harness: request.harness, project: request.project, resume: null, approvalOverride: 'alwaysAsk', trustOnce: false, utility: true, memoryChat: null },
      onEvent,
    });

    state = initialChatState(helper.key);
    for (const envelope of pending) {
      state = reduceEnvelope(state, envelope);
    }

    if (!(await until((current) => current.readyCount > 0))) {
      return null;
    }

    if (request.model) {
      await transport.invoke('agent_send', { key: helper.key, action: { type: 'setModel', provider: request.model.provider, modelId: request.model.id } });
    }

    const settledBefore = state.settledCount;
    await transport.invoke('agent_send', { key: helper.key, action: { type: 'prompt', message: namingPrompt(request.prompt), images: [], streamingBehavior: null } });

    if (!(await until((current) => current.settledCount > settledBefore))) {
      return null;
    }

    return parseLabel(replyText(state));
  } catch {
    return null;
  } finally {
    if (helper) {
      void transport.invoke('agent_stop', { key: helper.key }).catch(() => undefined);
    }
  }
}
