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

describe('workspace restore of the view the Memory view became', () => {
  /** The view a fresh launch shows after restoring a record saved on `view`. A launch restores once, so the stores are loaded afresh. */
  async function relaunchOn(view: string): Promise<string> {
    localStorage.setItem('harness:workspace', JSON.stringify({ view, chats: [{ harness: 'omp', sessionId: 'abc-123', project: PROJECT }], active: 0 }));
    vi.resetModules();
    const { restoreWorkspace: restore } = await import('./workspace.svelte');
    const { nav: relaunched } = await import('./nav.svelte');

    await restore(storeWithMock().store);

    return relaunched.current;
  }

  it('opens Notes for a record saved while the Memory view was on screen', async () => {
    expect(await relaunchOn('memory')).toBe('notes');
  });

  it('opens the chat for a view that does not exist any more', async () => {
    expect(await relaunchOn('gone')).toBe('chat');
  });
});

describe('workspace restore of a saved duplicate', () => {
  async function relaunch() {
    vi.resetModules();

    return import('./workspace.svelte');
  }

  function save(chats: { sessionId: string }[], active: number): void {
    localStorage.setItem('harness:workspace', JSON.stringify({ view: 'chat', chats: chats.map(({ sessionId }) => ({ harness: 'omp', sessionId, project: PROJECT })), active }));
  }

  it('starts one agent for a session saved twice and keeps the chat after it active', async () => {
    save([{ sessionId: 'dup' }, { sessionId: 'dup' }, { sessionId: 'other' }], 2);
    const { restoreWorkspace: restore } = await relaunch();
    const { store, of } = storeWithMock();

    expect(await restore(store)).toBe(true);

    expect(of('agent_start').map(({ request }) => request.resume?.id)).toEqual(['dup', 'other']);
    expect(store.sessions).toHaveLength(2);
    expect(store.activeKey).toBe(store.sessions[1].key);
  });

  it('shows the kept chat when the active one was the dropped duplicate', async () => {
    save([{ sessionId: 'dup' }, { sessionId: 'other' }, { sessionId: 'dup' }], 2);
    const { restoreWorkspace: restore } = await relaunch();
    const { store, of } = storeWithMock();

    await restore(store);

    expect(of('agent_start')).toHaveLength(2);
    expect(store.activeKey).toBe(store.sessions[0].key);
  });
});

describe('chat names across a restart', () => {
  it('keeps the helper\'s name and description, and that the user renamed the chat since', async () => {
    const { store } = storeWithMock();
    const chat = await store.create('omp', PROJECT);
    await vi.waitFor(() => expect(chat?.chat.session?.sessionId).toBeTruthy());
    chat!.chat.items = [{ kind: 'message', id: 'm1', role: 'user', blocks: [{ type: 'text', text: 'hi' }] } as never];
    chat!.aiLabel = { name: 'Dependency check', description: 'Which packages are outdated.' };
    chat!.userNamed = true;
    saveWorkspace(store);

    // A launch restores once: a fresh copy of the module stands for the next launch.
    vi.resetModules();
    const relaunched = await import('./workspace.svelte');
    const { store: next } = storeWithMock();
    await relaunched.restoreWorkspace(next);

    expect(next.sessions[0].aiLabel).toEqual({ name: 'Dependency check', description: 'Which packages are outdated.' });
    expect(next.sessions[0].userNamed).toBe(true);
    expect(next.description(next.sessions[0].key)).toBe('Which packages are outdated.');
  });

  it('ignores a damaged saved label', async () => {
    localStorage.setItem('harness:workspace', JSON.stringify({ view: 'chat', chats: [{ harness: 'omp', sessionId: 'abc', project: PROJECT, label: { name: 7 } }], active: 0 }));
    vi.resetModules();
    const relaunched = await import('./workspace.svelte');
    const { store } = storeWithMock();

    await relaunched.restoreWorkspace(store);

    expect(store.sessions[0].aiLabel).toBeNull();
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

describe('workspace restore of a harness without chat', () => {
  it('leaves out the saved chats of a harness the app cannot chat with, and reopens the others', async () => {
    vi.stubEnv('VITE_TRANSPORT', 'mock');
    localStorage.setItem(
      'harness:workspace',
      JSON.stringify({
        view: 'chat',
        chats: [
          { harness: 'opencode', sessionId: 'ses_gone', project: PROJECT },
          { harness: 'omp', sessionId: 'abc-123', project: PROJECT },
        ],
        active: 1,
      }),
    );
    vi.resetModules();
    const { harness } = await import('./harness.svelte');
    await harness.load();
    const { restoreWorkspace: restore } = await import('./workspace.svelte');
    const { store, of } = storeWithMock();

    expect(await restore(store)).toBe(true);

    expect(of('agent_start').map(({ request }) => request.harness)).toEqual(['omp']);
  });
});

describe('workspace restore of a harness this build does not know', () => {
  it('drops a saved chat naming an unknown harness and reopens the known ones', async () => {
    localStorage.setItem(
      'harness:workspace',
      JSON.stringify({
        view: 'chat',
        chats: [
          { harness: 'gemini', sessionId: 'ses_unknown', project: PROJECT },
          { harness: 'omp', sessionId: 'abc-123', project: PROJECT },
        ],
        active: 1,
      }),
    );
    vi.resetModules();
    const { restoreWorkspace: restore } = await import('./workspace.svelte');
    const { store, of } = storeWithMock();

    expect(await restore(store)).toBe(true);

    expect(of('agent_start').map(({ request }) => request.harness)).toEqual(['omp']);
  });
});
