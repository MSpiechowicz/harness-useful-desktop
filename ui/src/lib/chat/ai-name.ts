import type { AgentSessionInfo } from '$lib/ipc/bindings/AgentSessionInfo';
import type { Transport } from '$lib/ipc/transport';

import { withHelper } from './helper-session';
import { namingPrompt, parseLabel, type ChatLabel } from './naming';
import type { ChatState } from './types';

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
  return withHelper(transport, request, async (helper) => {
    if (request.model) {
      await helper.send({ type: 'setModel', provider: request.model.provider, modelId: request.model.id });
    }

    const settledBefore = helper.state().settledCount;
    await helper.send({ type: 'prompt', message: namingPrompt(request.prompt), images: [], streamingBehavior: null });

    return (await helper.until((current) => current.settledCount > settledBefore)) ? parseLabel(replyText(helper.state())) : null;
  });
}
