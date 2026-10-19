import type { ChatSession } from '$lib/chat/session.svelte';
import { harness } from '$lib/stores/harness.svelte';

/** Whether the chat has an agent picker: its harness can switch agents, and the chat has reported the agent it runs. */
export function hasAgentPicker(session: ChatSession): boolean {
  return Boolean(harness.get(session.info.harness)?.capabilities.agentPicker) && (session.chat.session?.agent ?? null) !== null;
}
