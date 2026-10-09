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
  it('loads preferences from the backend', async () => {
    const store = new PrefsStore(fakeTransport({ prefs_get: () => samplePrefs({ theme: 'dark' }) }), new ToastsStore());

    await store.load();

    expect(store.status).toBe('ready');
    expect(store.value?.theme).toBe('dark');
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
