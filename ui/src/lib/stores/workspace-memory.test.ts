import { afterEach, describe, expect, it } from 'vitest';

import type { AgentStartRequest } from '$lib/ipc/bindings/AgentStartRequest';

import { PROJECT, recordingTransport } from '../../../tests/chat-helpers';
import { AgentSessionsStore } from './agentSession.svelte';
import { MemoryChatStore } from './memoryChat.svelte';
import { ToastsStore } from './toasts.svelte';
import { restoreWorkspace, saveWorkspace } from './workspace.svelte';

const env = { ready: async () => {}, supports: () => true, askDefaults: () => ({ notes: false, scripts: false }) };

function launch() {
  const backend = recordingTransport();
  const toasts = new ToastsStore();
  const memory = new MemoryChatStore(backend.transport, toasts, env, null);

  return { ...backend, memory, store: new AgentSessionsStore(backend.transport, toasts, memory) };
}

afterEach(() => localStorage.clear());

describe('the workspace of a chat with a memory identity', () => {
  it('reopens the chat with the key and the guards it had, whatever the next run would have given it', async () => {
    const before = launch();
    const session = (await before.store.create('omp', PROJECT))!;
    await before.memory.setKind(session.key, 'scripts', { ask: true });
    // The conversation has begun, so the chat is one the workspace reopens.
    session.chat = {
      ...session.chat,
      session: { ...(session.chat.session as object), sessionId: 'session-1' } as never,
      items: [{ kind: 'message', key: 'm1', order: 0, role: 'user', blocks: [{ type: 'text', text: 'hello' }], streaming: false, slots: {}, meta: null }],
    };
    saveWorkspace(before.store);
    const started = before.of('agent_start')[0].request.memoryChat;

    const after = launch();
    expect(await restoreWorkspace(after.store)).toBe(true);

    const reopened = after.of('agent_start')[0].request as AgentStartRequest;
    expect(reopened.resume).toEqual({ harness: 'omp', id: 'session-1' });
    expect(reopened.memoryChat?.key).toBe(started?.key);
    expect(reopened.memoryChat?.guards.scripts.ask).toBe(true);
  });
});
