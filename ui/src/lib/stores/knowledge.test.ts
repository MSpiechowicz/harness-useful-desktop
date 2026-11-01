import { describe, expect, it } from 'vitest';

import type { KnowledgePulse } from '$lib/ipc/bindings/KnowledgePulse';
import type { KnowledgeStatus } from '$lib/ipc/bindings/KnowledgeStatus';
import type { MemoryEmbedderState } from '$lib/ipc/bindings/MemoryEmbedderState';
import { IpcFailure } from '$lib/ipc/transport';

import { fakeTransport, samplePrefs } from '../../../tests/helpers';
import { KnowledgeChangesStore } from './knowledgeChanges.svelte';
import { KnowledgeSearchStore } from './knowledgeEmbedder.svelte';
import { KnowledgeRemoteStore } from './knowledgeRemote.svelte';
import { KnowledgeStatusStore } from './knowledgeStatus.svelte';
import { PrefsStore } from './prefs.svelte';
import { ToastsStore } from './toasts.svelte';

const NO_FEATURES = { docs: false, import: false, embedder: false, search: false, recall: false, held: false, mcp: false };

describe('the live counts of the knowledge database', () => {
  it('shows the proposals waiting for review from the latest pulse, and nothing before the first one', () => {
    let send: (pulse: KnowledgePulse) => void = () => {};
    const store = new KnowledgeChangesStore(
      fakeTransport({
        knowledge_changes_listen: ({ onEvent }: { onEvent: (pulse: KnowledgePulse) => void }) => {
          send = onEvent;

          return null;
        },
      }),
    );

    const stop = store.start();
    expect(store.held).toBe(0);

    send({ documents: 12, generation: 3, held: 2, offline: false });
    expect(store.held).toBe(2);

    // A server without a review queue says nothing about one.
    send({ documents: 12, generation: 4, held: null, offline: false });
    expect(store.held).toBe(0);

    stop();
    send({ documents: 13, generation: 5, held: 7, offline: false });
    expect(store.pulse?.generation).toBe(4);
  });

  it('tells the backend whether the Knowledge view is open, so it reads fast only then', () => {
    const told: boolean[] = [];
    const store = new KnowledgeChangesStore(fakeTransport({ knowledge_changes_watch: ({ watching }: { watching: boolean }) => (told.push(watching), null) }));

    store.watch(true);
    store.watch(false);

    expect(told).toEqual([true, false]);
  });

  it('stays quiet when there is no server to follow', async () => {
    const store = new KnowledgeChangesStore(fakeTransport({ knowledge_changes_listen: () => Promise.reject(IpcFailure.from({ code: 'notFound', message: 'not installed' })) }));

    store.start();
    await Promise.resolve();

    expect(store.pulse).toBeNull();
  });
});

describe('what the settings offer for the installed Harness Knowledge', () => {
  const reading = (availability: KnowledgeStatus['availability']) => fakeTransport({ knowledge_status: () => ({ availability, features: NO_FEATURES }) });

  it('offers an install while it is missing and an update while it is too old, and nothing while unknown or current', async () => {
    const results: Record<string, 'install' | 'update' | null> = {};

    for (const type of ['notInstalled', 'outdated', 'available'] as const) {
      const store = new KnowledgeStatusStore(reading({ type } as KnowledgeStatus['availability']));
      expect(store.installable).toBeNull();

      await store.load();
      results[type] = store.installable;
    }

    expect(results).toEqual({ notInstalled: 'install', outdated: 'update', available: null });
  });

  it('keeps what it knew when the server cannot be asked', async () => {
    const store = new KnowledgeStatusStore(reading({ type: 'notInstalled' }));
    await store.load();

    const failing = new KnowledgeStatusStore(fakeTransport({ knowledge_status: () => Promise.reject(new Error('down')) }));
    await failing.load();

    expect(store.installable).toBe('install');
    expect(failing.installable).toBeNull();
  });
});

describe('the Project knowledge switch', () => {
  function prefsOver(handlers: Parameters<typeof fakeTransport>[0]) {
    const toasts = new ToastsStore();
    const store = new PrefsStore(fakeTransport(handlers), toasts);
    store.value = samplePrefs();

    return { store, toasts };
  }

  it('changes only knowledge, and keeps the entries a commented file left for the user to remove', async () => {
    const removal = { harness: 'opencode', file: '~/.config/opencode/opencode.jsonc', name: 'harness-knowledge' } as const;
    const { store } = prefsOver({ knowledge_set_enabled: () => ({ knowledgeEnabled: false, server: { installed: true, version: '1.0.0', supportedOnThisOs: true, command: 'x', sideEffects: [] }, manualRemovals: [removal] }) });

    await store.setKnowledgeEnabled(false);

    expect(store.value?.knowledgeEnabled).toBe(false);
    expect(store.value?.memoryEnabled).toBe(true);
    expect(store.knowledgeManualRemovals).toEqual([removal]);
    expect(store.memoryManualRemovals).toEqual([]);

    store.dismissKnowledgeManualRemovals();
    expect(store.knowledgeManualRemovals).toEqual([]);
  });

  it('leaves the switch as it was and says so when the backend refuses', async () => {
    const { store, toasts } = prefsOver({ knowledge_set_enabled: () => Promise.reject(IpcFailure.from({ code: 'internal', message: 'config is read-only' })) });

    await store.setKnowledgeEnabled(false);

    expect(store.value?.knowledgeEnabled).toBe(true);
    expect(store.knowledgePending).toBe(false);
    expect(toasts.items[0]?.description).toBe('config is read-only');
  });

  it('does not let a re-read undo a switch that is still in flight', async () => {
    let finish: () => void = () => {};
    const answer = new Promise<void>((resolve) => (finish = resolve));
    const { store } = prefsOver({
      knowledge_set_enabled: async () => {
        await answer;

        return { knowledgeEnabled: false, server: { installed: true, version: '1.0.0', supportedOnThisOs: true, command: 'x', sideEffects: [] }, manualRemovals: [] };
      },
      prefs_get: () => samplePrefs({ knowledgeEnabled: true, knowledgeVectorSearch: false }),
    });

    const switching = store.setKnowledgeEnabled(false);
    await store.syncServerManaged();

    // The pending switch decides its own pref, and the vector search pref still follows the backend.
    expect(store.value?.knowledgeEnabled).toBe(true);
    expect(store.value?.knowledgeVectorSearch).toBe(false);

    finish();
    await switching;
    expect(store.value?.knowledgeEnabled).toBe(false);
  });
});

describe('vector search of Harness Knowledge', () => {
  const ready: MemoryEmbedderState = { phase: 'ready', progress: null, model: 'e5', error: null, modelPresent: true };

  function storeWith(embedderSupported: boolean | null) {
    const toasts = new ToastsStore();
    const prefs = new PrefsStore(fakeTransport({}), toasts);
    prefs.value = samplePrefs();
    const remote = new KnowledgeRemoteStore(fakeTransport({}));
    remote.value = embedderSupported === null ? null : { enabled: false, kind: null, urlRedacted: null, hasToken: false, embedderSupported, repositoryDocuments: 'remote', globalDocuments: 'remote', routingSupported: true, configPath: '', problem: null };
    const calls: boolean[] = [];
    const transport = fakeTransport({
      knowledge_set_vector_search: ({ enabled }: { enabled: boolean }) => {
        calls.push(enabled);

        return { vectorSearch: enabled, state: ready };
      },
    });

    return { search: new KnowledgeSearchStore(transport, toasts, prefs, remote), prefs, calls };
  }

  it('is shown only once the server said it has an embedder', () => {
    expect(storeWith(null).search.vectorSupported).toBe(false);
    expect(storeWith(false).search.vectorSupported).toBe(false);
    expect(storeWith(true).search.vectorSupported).toBe(true);
  });

  it('switches the knowledge pref and not the memory one', async () => {
    const { search, prefs, calls } = storeWith(true);

    expect(search.vectorOn).toBe(true);
    await search.setVectorSearch(false);

    expect(calls).toEqual([false]);
    expect(prefs.value?.knowledgeVectorSearch).toBe(false);
    expect(prefs.value?.vectorSearch).toBe(true);
    expect(search.state).toEqual(ready);
    expect(prefs.knowledgeVectorSearchPending).toBe(false);
  });
});
