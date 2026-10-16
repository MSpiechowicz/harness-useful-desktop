import { describe, expect, it } from 'vitest';

import { fakeTransport, samplePrefs } from '../../../tests/helpers';
import { IpcFailure } from '$lib/ipc/transport';

import { PrefsStore } from './prefs.svelte';
import { ToastsStore } from './toasts.svelte';

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

describe('PrefsStore', () => {
  it('sets project memory through its own command and takes only that field from the answer', async () => {
    const calls: boolean[] = [];
    const store = new PrefsStore(
      fakeTransport({
        prefs_get: () => samplePrefs({ theme: 'dark' }),
        memory_set_enabled: ({ enabled }: { enabled: boolean }) => {
          calls.push(enabled);
          return { memoryEnabled: enabled, manualRemovals: [] };
        },
      }),
      new ToastsStore(),
    );
    await store.load();

    const pending = store.setMemoryEnabled(false);
    expect(store.memoryPending).toBe(true);
    await pending;

    expect(calls).toEqual([false]);
    expect(store.memoryPending).toBe(false);
    expect(store.value).toMatchObject({ memoryEnabled: false, theme: 'dark' });
  });

  it('keeps the entries the switch left to remove by hand until they are dismissed or memory is turned on again', async () => {
    const removal = { harness: 'opencode', file: '~/.config/opencode/opencode.jsonc', name: 'harness-memory' };
    const store = new PrefsStore(
      fakeTransport({
        prefs_get: () => samplePrefs(),
        memory_set_enabled: ({ enabled }: { enabled: boolean }) => ({ memoryEnabled: enabled, manualRemovals: enabled ? [] : [removal] }),
      }),
      new ToastsStore(),
    );
    await store.load();

    await store.setMemoryEnabled(false);
    expect(store.memoryManualRemovals).toEqual([removal]);

    store.dismissMemoryManualRemovals();
    expect(store.memoryManualRemovals).toEqual([]);

    await store.setMemoryEnabled(false);
    await store.setMemoryEnabled(true);
    expect(store.memoryManualRemovals).toEqual([]);
  });

  it('keeps project memory as it was and raises a toast when the command fails', async () => {
    const toasts = new ToastsStore();
    const store = new PrefsStore(
      fakeTransport({
        prefs_get: () => samplePrefs({ memoryEnabled: true }),
        memory_set_enabled: () => {
          throw { code: 'io', message: 'config is read-only' };
        },
      }),
      toasts,
    );
    await store.load();

    await store.setMemoryEnabled(false);

    expect(store.value?.memoryEnabled).toBe(true);
    expect(store.memoryPending).toBe(false);
    expect(toasts.items[0]).toMatchObject({ tone: 'danger', description: 'config is read-only' });
  });

  it('loads preferences from the backend', async () => {
    const store = new PrefsStore(fakeTransport({ prefs_get: () => samplePrefs({ theme: 'dark' }) }), new ToastsStore());

    await store.load();

    expect(store.status).toBe('ready');
    expect(store.value?.theme).toBe('dark');
  });

  it('shares one request between concurrent loads', async () => {
    const read = deferred<ReturnType<typeof samplePrefs>>();
    let calls = 0;
    const store = new PrefsStore(
      fakeTransport({
        prefs_get: () => {
          calls += 1;

          return read.promise;
        },
      }),
      new ToastsStore(),
    );

    const first = store.load();
    const second = store.load();
    read.resolve(samplePrefs({ theme: 'dark' }));
    await Promise.all([first, second]);

    expect(calls).toBe(1);
    expect(store.value?.theme).toBe('dark');

    await store.load();

    expect(calls).toBe(2);
  });

  it('reports a failed load without throwing', async () => {
    const failure = { code: 'io', message: 'disk unreadable' };
    const store = new PrefsStore(
      fakeTransport({
        prefs_get: () => {
          throw failure;
        },
      }),
      new ToastsStore(),
    );

    await store.load();

    expect(store.status).toBe('error');
    expect(store.error).toBeInstanceOf(IpcFailure);
    expect(store.error?.code).toBe('io');
  });

  it('applies an update immediately, then adopts the saved value', async () => {
    const save = deferred<ReturnType<typeof samplePrefs>>();
    const store = new PrefsStore(
      fakeTransport({ prefs_get: () => samplePrefs(), prefs_set: () => save.promise }),
      new ToastsStore(),
    );
    await store.load();

    const pending = store.update({ theme: 'light' });
    expect(store.value?.theme).toBe('light');

    save.resolve(samplePrefs({ theme: 'light', version: 2 }));
    await pending;

    expect(store.value?.version).toBe(2);
  });

  it('rolls back and shows a toast when saving fails', async () => {
    const toasts = new ToastsStore();
    const store = new PrefsStore(
      fakeTransport({
        prefs_get: () => samplePrefs({ theme: 'dark' }),
        prefs_set: () => {
          throw { code: 'io', message: 'read-only file system' };
        },
      }),
      toasts,
    );
    await store.load();

    const pending = store.update({ theme: 'light' });
    expect(store.value?.theme).toBe('light');
    await pending;

    expect(store.value?.theme).toBe('dark');
    expect(toasts.items).toHaveLength(1);
    expect(toasts.items[0]).toMatchObject({ tone: 'danger', description: 'read-only file system' });
  });

  it('keeps the newest value when an older save fails after a newer update', async () => {
    const first = deferred<ReturnType<typeof samplePrefs>>();
    const second = deferred<ReturnType<typeof samplePrefs>>();
    const calls = [first, second];
    const store = new PrefsStore(
      fakeTransport({ prefs_get: () => samplePrefs(), prefs_set: () => calls.shift()!.promise }),
      new ToastsStore(),
    );
    await store.load();

    const older = store.update({ theme: 'dark' });
    const newer = store.update({ motion: 'off' });
    first.reject({ code: 'io', message: 'boom' });
    second.resolve(samplePrefs({ theme: 'dark', motion: 'off' }));
    await Promise.all([older, newer]);

    expect(store.value).toMatchObject({ theme: 'dark', motion: 'off' });
  });
});
