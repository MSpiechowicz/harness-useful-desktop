import { afterEach, describe, expect, it, vi } from 'vitest';

import type { CommandName } from '$lib/ipc/commands';
import { MockMemoryGuards } from '$lib/ipc/mock/memory-guards-handlers';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { recordingTransport } from '../../../tests/chat-helpers';
import { GLOBAL_RETRY_MS, type MemoryChatEnv, MemoryChatStore } from './memoryChat.svelte';
import { ToastsStore } from './toasts.svelte';

const env: MemoryChatEnv = { ready: async () => {}, supports: () => true, askDefaults: () => ({ notes: false, scripts: false }) };

function storeOn(transport: Transport, toasts = new ToastsStore()) {
  return { toasts, store: new MemoryChatStore(transport, toasts, env, null) };
}

/** A backend of the guards alone, from the mock, that a test can break. */
function guardsTransport() {
  const guards = new MockMemoryGuards(() => 'empty');
  const handlers = guards.handlers() as unknown as Record<string, (args: unknown) => unknown>;
  const calls: CommandName[] = [];
  const transport: Transport = {
    async invoke(name, args) {
      calls.push(name);

      return handlers[name](args) as never;
    },
  };

  return { guards, transport, count: (name: CommandName) => calls.filter((call) => call === name).length };
}

/** The transport with its reads of the guards held back until `release()`, which a test starts with `hold()`. */
function gated(transport: Transport) {
  let gate: Promise<void> | null = null;
  let open = () => {};

  const hold = () => {
    gate = new Promise<void>((resolve) => (open = resolve));
  };

  return {
    hold,
    release: () => {
      gate = null;
      open();
    },
    transport: {
      async invoke(name, args) {
        // The answer is taken at the call, so a read that was started before an update holds the state from before it.
        const answer = transport.invoke(name, args);
        answer.catch(() => {});

        if (name === 'memory_guards_get') {
          await gate;
        }

        return answer;
      },
    } as Transport,
  };
}

afterEach(() => {
  vi.useRealTimers();
});

describe('the saving root cache', () => {
  /** Lets every answer that is ready be taken (the mock backend answers on a timer). */
  const settle = () => new Promise((resolve) => setTimeout(resolve, 10));

  it('asks once per look while nothing changes, whatever is read again, and once more after a guards update', async () => {
    const backend = recordingTransport();
    const { store } = storeOn(backend.transport);
    await store.loadGlobal();

    store.rootOf('/work/a');
    await settle();
    store.rootOf('/work/a');
    store.rootOf('/work/a');
    await store.loadGlobal();
    await store.loadGlobal();
    await settle();
    store.rootOf('/work/a');

    expect(backend.of('memory_saving_root')).toHaveLength(1);

    await store.setRepoOn('notes', '/work/a', true);
    await settle();
    store.rootOf('/work/a');
    store.rootOf('/work/a');

    expect(backend.of('memory_saving_root')).toHaveLength(2);
  });

  it('asks again when a read finds the guards changed elsewhere, and not when it finds them as they were', async () => {
    const backend = recordingTransport();
    const { store } = storeOn(backend.transport);
    await store.loadGlobal();
    store.rootOf('/work/a');
    await settle();

    await backend.transport.invoke('memory_guards_update', { patch: { kind: 'notes', addRepo: '/work/b' } });
    await store.loadGlobal();
    store.rootOf('/work/a');
    await settle();

    expect(backend.of('memory_saving_root')).toHaveLength(2);

    await store.loadGlobal();
    store.rootOf('/work/a');
    await settle();

    expect(backend.of('memory_saving_root')).toHaveLength(2);
  });

  it('does not ask again for the first read, which changed nothing a root depends on', async () => {
    const backend = recordingTransport();
    const { store } = storeOn(backend.transport);
    store.rootOf('/work/a');
    await settle();

    await store.loadGlobal();
    await settle();

    expect(backend.of('memory_saving_root')).toHaveLength(1);
  });

  it('keeps the old root until the new answer is in, so what depends on it does not blink', async () => {
    const answers = ['/work/a', '/work/main'];
    const pending: (() => void)[] = [];
    const backend = recordingTransport();
    const transport: Transport = {
      invoke(name, args) {
        if (name !== 'memory_saving_root') {
          return backend.transport.invoke(name, args);
        }

        const answer = answers.shift() as never;

        return pending.length === 0 && answer === '/work/a' ? Promise.resolve(answer) : new Promise((resolve) => pending.push(() => resolve(answer)));
      },
    };
    const { store } = storeOn(transport);
    await store.loadGlobal();
    store.rootOf('/work/a');
    await settle();

    await store.setRepoOn('notes', '/work/main', true);

    expect(store.rootOf('/work/a')).toBe('/work/a');
    await settle();
    expect(store.rootOf('/work/a')).toBe('/work/a');
    pending.splice(0).forEach((resolve) => resolve());
    await settle();
    expect(store.rootOf('/work/a')).toBe('/work/main');
  });

  it('drops an answer that was asked before an update landed and asks once more', async () => {
    const resolvers: ((root: string) => void)[] = [];
    const backend = recordingTransport();
    const transport: Transport = {
      invoke: (name, args) => (name === 'memory_saving_root' ? new Promise((resolve) => resolvers.push(resolve as (root: string) => void)) : backend.transport.invoke(name, args)),
    };
    const { store } = storeOn(transport);
    await store.loadGlobal();

    store.rootOf('/work/a');
    await store.setRepoOn('notes', '/work/a', true);
    resolvers[0]('/stale');
    await settle();

    expect(store.rootOf('/work/a')).toBeNull();
    expect(resolvers).toHaveLength(2);

    resolvers[1]('/work/fresh');
    await settle();

    expect(store.rootOf('/work/a')).toBe('/work/fresh');
  });

  it('does not keep a null answer, so a folder that comes back is found at the next look', async () => {
    let answer: string | null = null;
    const asked: string[] = [];
    const transport: Transport = {
      invoke(_name, args) {
        asked.push((args as { folder: string }).folder);

        return Promise.resolve(answer as never);
      },
    };
    const { store } = storeOn(transport);

    store.rootOf('/work/back');
    await settle();
    expect(store.rootOf('/work/back')).toBeNull();
    await settle();
    // One call per look: nothing else asked while no one looked.
    expect(asked).toEqual(['/work/back', '/work/back']);

    answer = '/work/back';
    store.rootOf('/work/back');
    await settle();

    expect(store.rootOf('/work/back')).toBe('/work/back');
    expect(asked).toHaveLength(3);
  });

  it('forgets the root of a folder that is gone after a guards update', async () => {
    let answer: string | null = '/work/a';
    const backend = recordingTransport();
    const transport: Transport = { invoke: (name, args) => (name === 'memory_saving_root' ? Promise.resolve(answer as never) : backend.transport.invoke(name, args)) };
    const { store } = storeOn(transport);
    await store.loadGlobal();
    store.rootOf('/work/a');
    await settle();
    expect(store.rootOf('/work/a')).toBe('/work/a');

    answer = null;
    await store.setRepoOn('notes', '/work/other', true);
    store.rootOf('/work/a');
    await settle();

    expect(store.rootOf('/work/a')).toBeNull();
  });
});

describe('the global guards that cannot be read or are still in the old file', () => {
  it('reads again by itself while watched, and stops asking once they are read', async () => {
    vi.useFakeTimers();
    const { guards, transport, count } = guardsTransport();
    const { store } = storeOn(transport);
    guards.failReads(new IpcFailure({ code: 'io', message: 'unreadable' }));

    const stop = store.watchGlobal();
    await vi.advanceTimersByTimeAsync(0);
    expect(store.globalFailed).toBe(true);
    expect(store.global).toBeNull();

    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);
    expect(store.globalFailed).toBe(true);
    expect(count('memory_guards_get')).toBe(2);

    guards.failReads(null);
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);

    expect(store.globalFailed).toBe(false);
    expect(store.global?.notes.paused).toBe(false);
    const reads = count('memory_guards_get');
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS * 4);
    expect(count('memory_guards_get')).toBe(reads);
    stop();
  });

  it('keeps reading while the old file is in use and takes it when it can be written, without a timer per watcher', async () => {
    vi.useFakeTimers();
    const { guards, transport, count } = guardsTransport();
    guards.setMigrationPending(true);
    const { store } = storeOn(transport);

    const stops = [store.watchGlobal(), store.watchGlobal()];
    await vi.advanceTimersByTimeAsync(0);
    expect(store.global?.migrationPending).toBe(true);
    expect(vi.getTimerCount()).toBe(1);
    const reads = count('memory_guards_get');

    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);
    expect(count('memory_guards_get')).toBe(reads + 1);

    guards.setMigrationPending(false);
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);

    expect(store.global?.migrationPending).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
    stops.forEach((stop) => stop());
  });

  it('stops reading when the last view stops watching', async () => {
    vi.useFakeTimers();
    const { guards, transport, count } = guardsTransport();
    guards.failReads(new IpcFailure({ code: 'io', message: 'unreadable' }));
    const { store } = storeOn(transport);
    const first = store.watchGlobal();
    const second = store.watchGlobal();
    await vi.advanceTimersByTimeAsync(0);

    first();
    first();
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS);
    const reads = count('memory_guards_get');
    expect(reads).toBeGreaterThan(1);

    second();
    expect(vi.getTimerCount()).toBe(0);
    await vi.advanceTimersByTimeAsync(GLOBAL_RETRY_MS * 3);
    expect(count('memory_guards_get')).toBe(reads);
  });

  it('keeps the object it has when a read finds the same guards', async () => {
    const { transport } = guardsTransport();
    const { store } = storeOn(transport);
    await store.loadGlobal();
    const first = store.global;

    await store.loadGlobal();

    expect(store.global).toBe(first);
  });

  it('does not start a read while one is under way, so a slow poll is not piled on', async () => {
    const { transport, count } = guardsTransport();
    const slow = gated(transport);
    const { store } = storeOn(slow.transport);

    slow.hold();

    const first = store.loadGlobal();
    const second = store.loadGlobal();

    expect(count('memory_guards_get')).toBe(1);

    slow.release();
    await Promise.all([first, second]);
    await store.loadGlobal();

    expect(count('memory_guards_get')).toBe(2);
  });

  it('drops a read that started before an update, so it cannot bring back what the update changed', async () => {
    const { transport } = guardsTransport();
    const slow = gated(transport);
    const { store } = storeOn(slow.transport);
    await store.loadGlobal();
    slow.hold();

    const slowRead = store.loadGlobal();
    await store.setGlobalPause('notes', true);
    expect(store.guards.notes.paused).toBe(true);
    slow.release();
    await slowRead;

    expect(store.guards.notes.paused).toBe(true);
  });

  it('drops a failed read that started before an update too', async () => {
    const { guards, transport } = guardsTransport();
    const slow = gated(transport);
    const { store } = storeOn(slow.transport);
    await store.loadGlobal();
    slow.hold();
    guards.failReads(new IpcFailure({ code: 'io', message: 'unreadable' }));

    const slowRead = store.loadGlobal();
    guards.failReads(null);
    await store.setGlobalPause('notes', true);
    slow.release();
    await slowRead;

    expect(store.globalFailed).toBe(false);
    expect(store.guards.notes.paused).toBe(true);
  });
});

describe('a refused change to the global guards', () => {
  it.each([
    ['protocol', 'settings are busy'],
    ['invalidInput', 'the guards file is over 256 KiB'],
  ] as const)('says why in the toast (%s) and leaves the guards as they were', async (code, message) => {
    const { guards, transport } = guardsTransport();
    const { store, toasts } = storeOn(transport);
    await store.loadGlobal();
    const before = store.global;
    guards.failUpdates(new IpcFailure({ code, message }));

    await store.setGlobalPause('notes', true);

    expect(toasts.items.map(({ title, description }) => ({ title, description }))).toEqual([{ title: 'Could not change the setting', description: message }]);
    expect(store.global).toBe(before);
    expect(store.guards.notes.paused).toBe(false);
  });
});
