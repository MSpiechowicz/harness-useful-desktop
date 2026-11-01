import { describe, expect, it, vi } from 'vitest';

import type { CodeIndexState } from '$lib/ipc/bindings/CodeIndexState';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { CodeGraphStore } from './codeGraph.svelte';
import { ToastsStore } from './toasts.svelte';

/** The Code graph setting of the preferences, which stores the choice and registers the server: it accepts unless the test says so. */
const setting = { setCodeEnabled: vi.fn(async (_on: boolean) => true) };

describe('the code index switch of the store', () => {
  const ON: CodeIndexState = { enabled: true, source: 'default' };

  function withReplies(replies: { get?: () => CodeIndexState; set?: (on: boolean) => CodeIndexState }) {
    const invoke = vi.fn(async (name: string, args?: { on?: boolean }) => {
      if (name === 'code_index_get') {
        return (replies.get ?? (() => ON))();
      }

      return (replies.set ?? ((on: boolean) => ({ enabled: on, source: 'file' as const })))(args?.on ?? false);
    });

    return { invoke, store: new CodeGraphStore({ invoke } as unknown as Transport, new ToastsStore(), Date.now, setting) };
  }

  it('shows nothing until the server answered, and nothing for a server without a code index', async () => {
    const missing = withReplies({
      get: () => {
        throw IpcFailure.from({ code: 'unsupported', message: 'no code index' });
      },
    });

    expect(missing.store.index).toBeNull();
    await missing.store.load();
    expect(missing.store.index).toBeNull();
  });

  it('knows Harness Code is missing or too old from why the index could not be read, and hides it for any other failure', async () => {
    const reads = (code: 'notFound' | 'unsupported' | 'process') =>
      withReplies({
        get: () => {
          throw IpcFailure.from({ code, message: 'no' });
        },
      }).store;
    const missing = reads('notFound');
    const old = reads('unsupported');
    const broken = reads('process');
    const ready = withReplies({}).store;

    await Promise.all([missing.load(), old.load(), broken.load(), ready.load()]);

    expect([missing.installable, old.installable, broken.installable, ready.installable]).toEqual(['install', 'update', null, null]);
    expect([missing.availability, old.availability, broken.availability, ready.availability]).toEqual(['notInstalled', 'outdated', null, 'ready']);
  });

  it('registers the server first, then turns the index on or off, and goes no further when the registration is refused', async () => {
    const order: string[] = [];
    setting.setCodeEnabled.mockImplementationOnce(async (on: boolean) => {
      order.push(`register:${on}`);

      return true;
    });
    const { store, invoke } = withReplies({});
    invoke.mockImplementation(async (name: string, args?: { on?: boolean }) => {
      order.push(name === 'code_index_set' ? `index:${args?.on}` : name);

      return name === 'code_index_get' ? ON : { enabled: args?.on ?? false, source: 'file' as const };
    });

    await store.load();
    await store.setEnabled(false);

    expect(order).toEqual(['code_index_get', 'register:false', 'index:false']);

    setting.setCodeEnabled.mockResolvedValueOnce(false);
    await store.setEnabled(true);

    expect(invoke).not.toHaveBeenCalledWith('code_index_set', { on: true });
    expect(store.index?.enabled).toBe(false);
  });

  it('stores the choice when Harness Code is not installed, and says nothing of the index it cannot reach', async () => {
    const { store } = withReplies({
      get: () => {
        throw IpcFailure.from({ code: 'notFound', message: 'not installed' });
      },
      set: () => {
        throw IpcFailure.from({ code: 'notFound', message: 'not installed' });
      },
    });

    await store.load();
    await store.setEnabled(true);

    expect(setting.setCodeEnabled).toHaveBeenLastCalledWith(true);
    expect(store.index).toBeNull();
    expect(store.failure).toBeNull();
    expect(store.availability).toBe('notInstalled');
  });

  it('still writes the index choice when the index was not known before (first read failed or is pending)', async () => {
    const { store, invoke } = withReplies({});

    await store.setEnabled(true);

    expect(invoke).toHaveBeenCalledWith('code_index_set', { on: true });
    expect(store.index).toEqual({ enabled: true, source: 'file' });
    expect(store.failure).toBeNull();
  });

  describe('when a step of the change fails', () => {
    it('changes nothing when the registration fails: no index call, the index and the error are as they were', async () => {
      const { store, invoke } = withReplies({});

      await store.load();
      setting.setCodeEnabled.mockResolvedValueOnce(false);
      await store.setEnabled(false);

      expect(invoke).not.toHaveBeenCalledWith('code_index_set', expect.anything());
      expect(store.index).toEqual(ON);
      expect(store.failure).toBeNull();
      expect(store.pending).toBe(false);
    });

    it('keeps the stored choice, shows the error and keeps the index the server still has when the index step fails, and a retry finishes it', async () => {
      let refuse = true;
      const { store, invoke } = withReplies({
        set: (on) => {
          if (refuse) {
            throw IpcFailure.from({ code: 'process', message: 'The settings file is read-only' });
          }

          return { enabled: on, source: 'file' };
        },
      });
      setting.setCodeEnabled.mockClear();

      await store.load();
      await store.setEnabled(false);

      expect(setting.setCodeEnabled).toHaveBeenCalledTimes(1);
      expect(store.index).toEqual(ON);
      expect(store.failure).toBe('The settings file is read-only');
      expect(store.pending).toBe(false);

      refuse = false;
      await store.setEnabled(false);

      expect(setting.setCodeEnabled).toHaveBeenCalledTimes(2);
      expect(setting.setCodeEnabled).toHaveBeenLastCalledWith(false);
      expect(invoke.mock.calls.filter(([name]) => name === 'code_index_set')).toHaveLength(2);
      expect(store.index).toEqual({ enabled: false, source: 'file' });
      expect(store.failure).toBeNull();
    });
  });

  it('switches at once and keeps what the server reports', async () => {
    const { store } = withReplies({});

    await store.load();
    const done = store.setEnabled(false);

    expect(store.index?.enabled).toBe(false);

    await done;
    expect(store.index).toEqual({ enabled: false, source: 'file' });
  });

  it('goes back and says why when the server refuses', async () => {
    const { store } = withReplies({
      set: () => {
        throw IpcFailure.from({ code: 'process', message: 'The settings file is read-only' });
      },
    });

    await store.load();
    await store.setEnabled(false);

    expect(store.index).toEqual(ON);
    expect(store.failure).toBe('The settings file is read-only');
  });

  it('does not let a read that began before a change overwrite it', async () => {
    let answer: (state: CodeIndexState) => void = () => undefined;
    const invoke = vi.fn((name: string, args?: { on?: boolean }) => {
      if (name === 'code_index_get') {
        return invoke.mock.calls.length === 1 ? Promise.resolve(ON) : new Promise<CodeIndexState>((resolve) => (answer = resolve));
      }

      return Promise.resolve({ enabled: args?.on ?? false, source: 'file' as const });
    });
    const store = new CodeGraphStore({ invoke } as unknown as Transport, new ToastsStore(), Date.now, setting);

    await store.load();

    const stale = store.load();
    await store.setEnabled(false);
    answer(ON);
    await stale;

    expect(store.index).toEqual({ enabled: false, source: 'file' });
  });

  it('does not change a switch the environment decides', async () => {
    const { store, invoke } = withReplies({ get: () => ({ enabled: false, source: 'env' }) });

    await store.load();
    await store.setEnabled(true);

    expect(invoke).not.toHaveBeenCalledWith('code_index_set', expect.anything());
    expect(store.index?.enabled).toBe(false);
  });
});
