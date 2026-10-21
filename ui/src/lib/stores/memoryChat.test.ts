import { describe, expect, it } from 'vitest';

import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { PROJECT, recordingTransport } from '../../../tests/chat-helpers';
import { AgentSessionsStore } from './agentSession.svelte';
import { defaultGuards } from './memory-chat';
import { type MemoryChatEnv, MemoryChatStore } from './memoryChat.svelte';
import { ToastsStore } from './toasts.svelte';

/** A storage that lives as long as the test, so a "relaunch" can read what the last run kept. */
function memoryStorage(): Storage {
  const data = new Map<string, string>();

  return {
    get length() {
      return data.size;
    },
    clear: () => data.clear(),
    getItem: (key) => data.get(key) ?? null,
    key: (index) => [...data.keys()][index] ?? null,
    removeItem: (key) => void data.delete(key),
    setItem: (key, value) => void data.set(key, value),
  };
}

function env(overrides: Partial<MemoryChatEnv> = {}): MemoryChatEnv {
  return { ready: async () => {}, supports: () => true, askDefaults: () => ({ notes: false, scripts: false }), ...overrides };
}

const KEY = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe('a chat’s memory identity', () => {
  it('gives each new chat its own lowercase key and starts it with everything on', async () => {
    const store = new MemoryChatStore(recordingTransport().transport, new ToastsStore(), env(), memoryStorage());

    const first = await store.launch('omp');
    const second = await store.launch('omp');

    expect(first?.key).toMatch(KEY);
    expect(second?.key).toMatch(KEY);
    expect(first?.key).not.toBe(second?.key);
    expect(first?.guards).toEqual({ paused: false, notes: { on: true, ask: false }, scripts: { on: true, ask: false } });
  });

  it('starts new chats with "Ask before saving" as the preferences say, per kind', async () => {
    const store = new MemoryChatStore(recordingTransport().transport, new ToastsStore(), env({ askDefaults: () => ({ notes: true, scripts: false }) }), memoryStorage());

    const start = await store.launch('omp');

    expect(start?.guards).toEqual(defaultGuards({ notes: true, scripts: false }));
  });

  it('gives a chat no identity where chats cannot carry guards (the harness, Harness Memory or project memory says no)', async () => {
    const store = new MemoryChatStore(recordingTransport().transport, new ToastsStore(), env({ supports: (harness) => harness !== 'omp' }), memoryStorage());

    expect(await store.launch('omp')).toBeNull();
    expect(await store.launch('pi')).not.toBeNull();
  });

  it('waits until it is known whether chats can carry guards', async () => {
    let known = false;
    const store = new MemoryChatStore(
      recordingTransport().transport,
      new ToastsStore(),
      env({
        ready: async () => {
          known = true;
        },
        supports: () => known,
      }),
      memoryStorage(),
    );

    expect(await store.launch('omp')).not.toBeNull();
  });

  it('keeps the key and the guards when the chat is restarted, brought back or resumed', async () => {
    const storage = memoryStorage();
    const first = new MemoryChatStore(recordingTransport().transport, new ToastsStore(), env(), storage);
    const start = await first.launch('omp');
    first.attach('agent-1', start!);
    await first.setPaused('agent-1', true);

    // A restart keeps what the running chat has now.
    const restarted = await first.launch('omp', { keep: first.get('agent-1') });
    expect(restarted).toEqual({ key: start!.key, guards: { ...start!.guards, paused: true } });

    // The session the agent wrote is remembered by harness and id, and found again by a later run that only knows the session.
    first.remember('agent-1', 'omp', 'session-9');
    const relaunched = new MemoryChatStore(recordingTransport().transport, new ToastsStore(), env(), storage);
    const resumed = await relaunched.launch('omp', { resume: { harness: 'omp', id: 'session-9' } });
    expect(resumed).toEqual({ key: start!.key, guards: { ...start!.guards, paused: true } });

    // Another session starts its own chat.
    const other = await relaunched.launch('omp', { resume: { harness: 'omp', id: 'session-10' } });
    expect(other?.key).not.toBe(start!.key);
  });

  it('does not take a damaged record for an identity', async () => {
    const storage = memoryStorage();
    storage.setItem('harness:memory-chats', JSON.stringify({ 'omp:s1': { key: 'Not-A-Key', guards: {} }, 'omp:s2': 'nope' }));
    const store = new MemoryChatStore(recordingTransport().transport, new ToastsStore(), env(), storage);

    const resumed = await store.launch('omp', { resume: { harness: 'omp', id: 's1' } });

    expect(resumed?.key).toMatch(KEY);
    expect(resumed?.key).not.toBe('Not-A-Key');
  });
});

describe('changing what a chat may save', () => {
  async function running(transport: Transport) {
    const toasts = new ToastsStore();
    const store = new MemoryChatStore(transport, toasts, env(), memoryStorage());
    const start = await store.launch('omp');
    store.attach('agent-1', start!);

    return { store, toasts, key: start!.key };
  }

  it('tells Harness Memory the chat’s new guards under its key', async () => {
    const backend = recordingTransport();
    const { store, key } = await running(backend.transport);

    await store.setKind('agent-1', 'scripts', { ask: true });
    await store.setPaused('agent-1', true);

    expect(backend.of('memory_chat_guards_set').map(({ key, guards }) => ({ key, guards }))).toEqual([
      { key, guards: { paused: false, notes: { on: true, ask: false }, scripts: { on: true, ask: true } } },
      { key, guards: { paused: true, notes: { on: true, ask: false }, scripts: { on: true, ask: true } } },
    ]);
  });

  it('numbers every write of a chat higher than the last, even for quick toggles, so a late one cannot win', async () => {
    const backend = recordingTransport();
    const { store } = await running(backend.transport);

    await Promise.all([store.setPaused('agent-1', true), store.setKind('agent-1', 'notes', { ask: true }), store.setPaused('agent-1', false), store.setKind('agent-1', 'scripts', { on: false })]);

    const revisions = backend.of('memory_chat_guards_set').map(({ revision }) => revision);
    expect(revisions).toHaveLength(4);
    expect(revisions.every((revision, index) => index === 0 || revision > revisions[index - 1])).toBe(true);
    expect(store.get('agent-1')?.guards).toEqual({ paused: false, notes: { on: true, ask: true }, scripts: { on: false, ask: false } });
  });

  it('writes once more above the newer revision Harness Memory already has, when a write was late', async () => {
    const calls: { revision: number; paused: boolean }[] = [];
    const transport: Transport = {
      async invoke(name, args) {
        const { revision, guards } = args as { revision: number; guards: { paused: boolean } };
        calls.push({ revision, paused: guards.paused });

        return calls.length === 1 ? { applied: false, revision: revision + 5000 } : { applied: true, revision };
      },
    };
    const { store } = await running(transport);

    await store.setPaused('agent-1', true);

    expect(calls).toHaveLength(2);
    expect(calls[1].revision).toBeGreaterThan(calls[0].revision + 5000);
    expect(calls[1].paused).toBe(true);
    expect(store.get('agent-1')?.guards.paused).toBe(true);
  });

  it('puts the switch back and says so when Harness Memory cannot be told', async () => {
    const failing: Transport = { invoke: () => Promise.reject(new IpcFailure({ code: 'io', message: 'the data folder is read-only' })) };
    const { store, toasts } = await running(failing);

    await store.setPaused('agent-1', true);

    expect(store.get('agent-1')?.guards.paused).toBe(false);
    expect(toasts.items.map((toast) => toast.description)).toEqual(['the data folder is read-only']);
  });

  it('adds a repository to the ones a kind is off in, and takes it away again', async () => {
    const backend = recordingTransport();
    const { store } = await running(backend.transport);

    await store.setRepoOff('notes', '/work/repo', true);
    expect(store.guards.notes.offRepos).toEqual(['/work/repo']);
    expect(store.guards.scripts.offRepos).toEqual([]);

    await store.setRepoOff('notes', '/work/repo', false);
    expect(store.guards.notes.offRepos).toEqual([]);
  });

  it('applies each quick global change to what is saved, so none is lost', async () => {
    const backend = recordingTransport();
    const { store } = await running(backend.transport);

    await Promise.all([store.setGlobalPause('notes', true), store.setRepoOff('scripts', '/work/a', true), store.setRepoOff('scripts', '/work/b', true)]);
    await store.loadGlobal();

    expect(store.guards).toEqual({ notes: { paused: true, offRepos: [] }, scripts: { paused: false, offRepos: ['/work/a', '/work/b'] } });
  });

  it('takes the state the backend answers with, not one worked out from an older read', async () => {
    const backend = recordingTransport();
    const { store } = await running(backend.transport);
    await store.loadGlobal();
    // Another window paused scripts meanwhile.
    await backend.transport.invoke('memory_guards_update', { patch: { kind: 'scripts', paused: true } });

    await store.setGlobalPause('notes', true);

    expect(store.guards).toEqual({ notes: { paused: true, offRepos: [] }, scripts: { paused: true, offRepos: [] } });
  });
});

describe('sessions starting an agent', () => {
  function sessions(overrides: Partial<MemoryChatEnv> = {}) {
    const backend = recordingTransport();
    const toasts = new ToastsStore();
    const memory = new MemoryChatStore(backend.transport, toasts, env(overrides), memoryStorage());

    return { ...backend, memory, store: new AgentSessionsStore(backend.transport, toasts, memory) };
  }

  it('starts a chat with its identity, and a helper chat with none', async () => {
    const { store, of } = sessions();

    await store.create('omp', PROJECT);
    await store.start({ harness: 'omp', project: PROJECT, resume: null, approvalOverride: null, trustOnce: false, utility: true, memoryChat: null });

    const [chat, helper] = of('agent_start');
    expect(chat.request.memoryChat?.key).toMatch(KEY);
    expect(helper.request.memoryChat).toBeNull();
  });

  it('starts a chat with no identity when guards are not supported', async () => {
    const { store, of } = sessions({ supports: () => false });

    await store.create('omp', PROJECT);

    expect(of('agent_start')[0].request.memoryChat).toBeNull();
    expect(store.sessions).toHaveLength(1);
  });

  it('keeps the key and the guards of a chat whose agent is restarted', async () => {
    const { store, memory, of } = sessions();
    const session = await store.create('omp', PROJECT);
    await memory.setKind(session!.key, 'notes', { ask: true });

    const fresh = await store.restart(session!.key, { trustOnce: true });

    const [first, second] = of('agent_start').map(({ request }) => request.memoryChat);
    expect(second?.key).toBe(first?.key);
    expect(second?.guards.notes.ask).toBe(true);
    expect(memory.get(fresh!.key)?.key).toBe(first?.key);
    expect(memory.get(session!.key)).toBeNull();
  });
});
