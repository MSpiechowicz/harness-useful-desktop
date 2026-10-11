import type { ChatDefaults } from '$lib/ipc/bindings/ChatDefaults';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { ModelOption } from '$lib/ipc/bindings/ModelOption';
import { prefs } from '$lib/stores/prefs.svelte';

import type { ChatSession } from './session.svelte';

/**
 * The model and thinking level a harness's new chats start with: the last ones picked in the app. They live in the
 * app's prefs only; the harness's own configuration is never written.
 */
export function savedChatDefaults(harness: HarnessId): ChatDefaults | null {
  return prefs.value?.chatDefaults.find((entry) => entry.harness === harness) ?? null;
}

function remember(harness: HarnessId, patch: Partial<Omit<ChatDefaults, 'harness'>>): void {
  const all = prefs.value?.chatDefaults ?? [];
  const current = savedChatDefaults(harness) ?? { harness, provider: null, modelId: null, thinkingLevel: null };

  void prefs.update({ chatDefaults: [...all.filter((entry) => entry.harness !== harness), { ...current, ...patch }] });
}

export function rememberModel(harness: HarnessId, model: ModelOption): void {
  remember(harness, { provider: model.provider, modelId: model.id });
}

export function rememberThinking(harness: HarnessId, level: string): void {
  remember(harness, { thinkingLevel: level });
}

/**
 * Puts a new chat on the saved model, then the saved thinking level (a model switch can reset it). A model or level
 * the agent no longer offers is skipped, so the chat keeps the agent's own default.
 */
export async function applyChatDefaults(session: ChatSession): Promise<void> {
  const saved = savedChatDefaults(session.info.harness);
  if (!saved) {
    return;
  }

  const { provider, modelId, thinkingLevel } = saved;
  const current = session.chat.session?.model;
  if (provider && modelId && (current?.provider !== provider || current.id !== modelId)) {
    await session.loadModels();
    const model = session.models?.find((option) => option.provider === provider && option.id === modelId);
    if (model) {
      await session.chooseModel(model);
    }
  }

  if (thinkingLevel && session.chat.session?.thinkingLevel !== thinkingLevel) {
    await session.loadThinkingLevels();
    if (session.thinkingLevels?.includes(thinkingLevel)) {
      await session.chooseThinking(thinkingLevel);
    }
  }
}
