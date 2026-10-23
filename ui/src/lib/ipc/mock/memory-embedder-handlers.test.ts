import { afterEach, describe, expect, it } from 'vitest';

import { memoryEmbedderHandlers } from './memory-embedder-handlers';
import { memoryRemoteHandlers } from './memory-remote-handlers';
import { MockState } from './state';

afterEach(() => {
  window.history.replaceState(null, '', '/');
});

function watched(handlers: ReturnType<typeof memoryEmbedderHandlers>) {
  const seen: unknown[] = [];
  handlers.memory_embedder_watch({ onEvent: (state) => seen.push(state) });

  return seen.at(-1);
}

describe('mock feature lines', () => {
  it('lists embedder, settings and notes-lang like a current Harness Memory', async () => {
    const remote = await memoryRemoteHandlers().memory_remote_get(undefined);

    expect([remote.embedderSupported, remote.settingsSupported, remote.notesLangSupported]).toEqual([true, true, true]);
  });

  it('lists neither embedder nor settings for ?embedder=old, and not notes-lang for ?lang=old', async () => {
    window.history.replaceState(null, '', '/?embedder=old');
    const noEmbedder = await memoryRemoteHandlers().memory_remote_get(undefined);

    expect([noEmbedder.embedderSupported, noEmbedder.settingsSupported, noEmbedder.notesLangSupported]).toEqual([false, false, true]);

    window.history.replaceState(null, '', '/?lang=old');
    const noLang = await memoryRemoteHandlers().memory_remote_get(undefined);

    expect([noLang.embedderSupported, noLang.settingsSupported, noLang.notesLangSupported]).toEqual([true, true, false]);
  });
});

describe('mock embedder model', () => {
  it('keeps the downloaded model when vector search goes off, until it is removed, and downloads it again with the switch on', async () => {
    const handlers = memoryEmbedderHandlers(new MockState(), () => 'ready');

    expect(watched(handlers)).toMatchObject({ phase: 'ready', modelPresent: true });
    expect((await handlers.memory_set_vector_search({ enabled: false })).state).toMatchObject({ phase: 'off', modelPresent: true });
    expect(await handlers.memory_embedder_remove_model(undefined)).toMatchObject({ phase: 'off', modelPresent: false });
    expect((await handlers.memory_set_vector_search({ enabled: true })).state).toMatchObject({ phase: 'ready', modelPresent: true });
    expect((await handlers.memory_set_vector_search({ enabled: false })).state).toMatchObject({ modelPresent: true });
  });

  it('does not claim to know about a model before the embedder got far enough', () => {
    const state = new MockState();
    state.setVectorSearch(false);

    expect(watched(memoryEmbedderHandlers(state, () => 'downloading'))).toMatchObject({ phase: 'off', modelPresent: false });
    expect(watched(memoryEmbedderHandlers(state, () => 'starting'))).toMatchObject({ phase: 'off', modelPresent: null });
  });
});
