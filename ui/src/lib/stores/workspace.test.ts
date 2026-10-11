import { afterEach, describe, expect, it, vi } from 'vitest';

import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { PROJECT, storeWithMock } from '../../../tests/chat-helpers';
import { AgentSessionsStore } from './agentSession.svelte';
import { nav } from './nav.svelte';
import { restoreWorkspace, saveWorkspace } from './workspace.svelte';

afterEach(() => {
  localStorage.clear();
  nav.go('chat');
});

describe('workspace restore', () => {
  it('reopens the last run’s chats, resuming their sessions, on the page the user left', async () => {
    localStorage.setItem(
      'harness:workspace',
      JSON.stringify({ view: 'changes', chats: [{ harness: 'omp', sessionId: 'abc-123', project: PROJECT }], active: 0 }),
    );
    const { store, of } = storeWithMock();

    expect(await restoreWorkspace(store)).toBe(true);

    expect(of('agent_start')[0]).toMatchObject({ request: { harness: 'omp', project: PROJECT, resume: { harness: 'omp', id: 'abc-123' } } });
    expect(store.sessions).toHaveLength(1);
    expect(store.activeKey).toBe(store.sessions[0].key);
    expect(nav.current).toBe('changes');
  });
});

describe('workspace save', () => {
  function saved(): { chats: { sessionId: string }[] } {
    return JSON.parse(localStorage.getItem('harness:workspace') ?? 'null');
  }

  it('keeps only chats with a message: the agent saves nothing before one, so the rest could not be resumed', async () => {
    const { store } = storeWithMock();
    const untouched = await store.create('omp', PROJECT);
    const talked = await store.create('omp', PROJECT);
    await vi.waitFor(() => expect(talked?.chat.session?.sessionId).toBeTruthy());
    // An extension notice is not a conversation.
    untouched!.chat.items = [{ kind: 'notice', id: 'n1', level: 'warning', message: 'Update available' } as never];
    talked!.chat.items = [{ kind: 'message', id: 'm1', role: 'user', blocks: [{ type: 'text', text: 'hi' }] } as never];

    saveWorkspace(store);

    expect(saved().chats.map((chat) => chat.sessionId)).toEqual([talked!.chat.session!.sessionId]);
  });
});

describe('workspace restore of a session that is gone', () => {
  it('leaves that chat closed without an error toast', async () => {
    vi.resetModules();
    const { restoreWorkspace: restore } = await import('./workspace.svelte');
    const { ToastsStore } = await import('./toasts.svelte');
    localStorage.setItem('harness:workspace', JSON.stringify({ view: 'chat', chats: [{ harness: 'omp', sessionId: 'gone', project: PROJECT }], active: 0 }));
    const transport: Transport = {
      invoke: (name) =>
        name === 'agent_start'
          ? Promise.reject(new IpcFailure({ code: 'process', message: 'Session "gone" not found.' }))
          : Promise.resolve([] as never),
    };
    const toasts = new ToastsStore();

    expect(await restore(new AgentSessionsStore(transport, toasts))).toBe(false);
    expect(toasts.items).toHaveLength(0);
  });
});
