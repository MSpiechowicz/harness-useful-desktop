import type { AgentStartRequest } from '$lib/ipc/bindings/AgentStartRequest';

import type { ChatMemory, memoryChat } from './memoryChat.svelte';

/** The part of the memory identities the sessions use: an agent gets its chat's identity when it starts, and gives it up when it stops. */
export interface MemoryIdentities {
  launch: typeof memoryChat.launch;
  attach: typeof memoryChat.attach;
  release: typeof memoryChat.release;
  get: typeof memoryChat.get;
  remember: typeof memoryChat.remember;
}

/**
 * The request with the chat's memory identity in it. `memoryChat` null in a request means "decide": the chat being replaced or brought
 * back (`keep`), else the session being resumed, else a new chat gets its own. A request that already has one keeps it, and a helper
 * agent (`utility`) never gets one.
 */
export async function withMemory(memory: MemoryIdentities, request: AgentStartRequest, keep: ChatMemory | null): Promise<AgentStartRequest> {
  if (request.utility || request.memoryChat) {
    return request;
  }

  return { ...request, memoryChat: await memory.launch(request.harness, { resume: request.resume, keep }) };
}
