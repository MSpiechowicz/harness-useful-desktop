import { memoryChat } from "$lib/stores/memoryChat.svelte";
import { memoryHeld } from "$lib/stores/memoryHeld.svelte";

/** Whether saves wait for the user's review in this chat, so the list pins a row for them after the last turn. Reads the stores, so call it where a change should show. */
export function hasMemoryRow(agentKey: string): boolean {
  return memoryHeld.of(memoryChat.get(agentKey)?.key ?? null).length > 0;
}
