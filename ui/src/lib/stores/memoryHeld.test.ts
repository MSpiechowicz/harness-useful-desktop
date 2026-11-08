import { describe, expect, it, vi } from 'vitest';

import { heldTitle } from '$lib/chat/memory-held';
import { HELD_NOTE_SAVE_ID, HELD_NOTE_UPDATE_ID, sampleHeld } from '$lib/ipc/mock/memory-guards-fixtures';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { recordingTransport } from '../../../tests/chat-helpers';
import { MemoryHeldStore } from './memoryHeld.svelte';

const FIRST = '11111111-2222-4333-8444-555555555555';
const SECOND = '66666666-7777-4888-8999-aaaaaaaaaaaa';
/** Toasts nobody looks at. */
const QUIET = { push: () => 0 };

describe('the waiting saves, per chat', () => {
  it('lists each open chat by its own key and stops reading a chat that closed', async () => {
    const backend = recordingTransport();
    const store = new MemoryHeldStore(backend.transport, QUIET);

    store.follow([FIRST, SECOND]);
    await store.load();
    expect(new Set(backend.of('memory_held_list').map(({ chatKey }) => chatKey))).toEqual(new Set([FIRST, SECOND]));

    store.items = [...sampleHeld(FIRST), ...sampleHeld(SECOND)];
    store.follow([SECOND]);

    expect(store.of(FIRST)).toEqual([]);
    expect(store.of(SECOND)).toHaveLength(4);

    backend.calls.length = 0;
    await store.load();

    expect(backend.of('memory_held_list')).toEqual([{ chatKey: SECOND }]);
  });
});

describe('saving a waiting save', () => {
  /** A backend whose save refuses as `error` and whose list answers with `items`, newest read last. */
  function refusing(code: string, items: () => ReturnType<typeof sampleHeld>) {
    const calls: { name: string; args: unknown }[] = [];
    const transport: Transport = {
      async invoke(name, args) {
        calls.push({ name, args });

        if (name === 'memory_held_save') {
          throw new IpcFailure({ code: code as 'changed', message: 'refused' });
        }

        return name === 'memory_held_list' ? items() : null;
      },
    };

    return { transport, calls };
  }

  it('sends the revision it showed, so the backend saves only that', async () => {
    const backend = recordingTransport();
    const store = new MemoryHeldStore(backend.transport, QUIET);
    const item = sampleHeld(FIRST).find((held) => held.heldId === HELD_NOTE_SAVE_ID)!;
    store.items = [item];

    await store.save(item);

    expect(backend.of('memory_held_save')).toEqual([{ heldId: HELD_NOTE_SAVE_ID, edited: null, revision: item.revision }]);
  });

  it('keeps the item, says the agent changed it and reads the chat’s list again', async () => {
    const older = sampleHeld(FIRST).find((held) => held.heldId === HELD_NOTE_UPDATE_ID)!;
    const newer = { ...older, revision: 'f'.repeat(64), arguments: { ...(older.arguments as object), body: 'The agent’s newer body.' } } as typeof older;
    const backend = refusing('changed', () => [newer]);
    const store = new MemoryHeldStore(backend.transport, QUIET);
    store.follow([FIRST]);
    await store.load();
    store.items = [older];
    backend.calls.length = 0;

    expect(await store.save(older)).toBe(false);

    await Promise.resolve();
    expect(store.failures[HELD_NOTE_UPDATE_ID]).toBe('The agent changed it. Review it again.');
    expect(backend.calls.filter((call) => call.name === 'memory_held_list')).toEqual([{ name: 'memory_held_list', args: { chatKey: FIRST } }]);
    await expect.poll(() => store.of(FIRST)).toEqual([newer]);
  });

  it('sends the revision it was given for an edited save, not the item’s newer one', async () => {
    const backend = recordingTransport();
    const store = new MemoryHeldStore(backend.transport, QUIET);
    const item = sampleHeld(FIRST).find((held) => held.heldId === HELD_NOTE_SAVE_ID)!;
    store.items = [item];

    await store.save({ ...item, revision: 'f'.repeat(64) }, { title: 'Edited' }, item.revision);

    expect(backend.of('memory_held_save')).toEqual([{ heldId: HELD_NOTE_SAVE_ID, edited: { title: 'Edited' }, revision: item.revision }]);
  });
});

describe('skipping a waiting save', () => {
  it('sends the revision it showed, so the backend drops only that', async () => {
    const backend = recordingTransport();
    const store = new MemoryHeldStore(backend.transport, QUIET);
    const item = sampleHeld(FIRST).find((held) => held.heldId === HELD_NOTE_SAVE_ID)!;
    store.items = [item];

    expect(await store.skip(item)).toBe(true);

    expect(backend.of('memory_held_skip')).toEqual([{ heldId: HELD_NOTE_SAVE_ID, revision: item.revision }]);
  });

  it('keeps the item, says the agent changed it and reads the chat’s list again, as for a save', async () => {
    const older = sampleHeld(FIRST).find((held) => held.heldId === HELD_NOTE_UPDATE_ID)!;
    const newer = { ...older, revision: 'f'.repeat(64) };
    const calls: string[] = [];
    const transport: Transport = {
      async invoke(name) {
        calls.push(name);

        if (name === 'memory_held_skip') {
          throw new IpcFailure({ code: 'changed', message: 'refused' });
        }

        return name === 'memory_held_list' ? [newer] : null;
      },
    };
    const store = new MemoryHeldStore(transport, QUIET);
    store.follow([FIRST]);
    store.items = [older];

    expect(await store.skip(older)).toBe(false);

    expect(store.failures[HELD_NOTE_UPDATE_ID]).toBe('The agent changed it. Review it again.');
    await expect.poll(() => store.of(FIRST)).toEqual([newer]);
  });
});

describe('after an action', () => {
  /** A backend whose list answers are held until the test lets them through. */
  function slowLists() {
    const reads: { chatKey: string; answer: (items: ReturnType<typeof sampleHeld>) => void }[] = [];
    const transport: Transport = {
      invoke(name, args) {
        if (name === 'memory_held_list') {
          return new Promise((resolve) => reads.push({ chatKey: (args as { chatKey: string }).chatKey, answer: resolve as never }));
        }

        return Promise.resolve(null);
      },
    };

    return { transport, reads };
  }

  it('does not read a chat that was closed meanwhile', async () => {
    const backend = recordingTransport();
    const store = new MemoryHeldStore(backend.transport, QUIET);
    const item = sampleHeld(FIRST)[0];
    store.follow([FIRST]);
    store.items = [item];
    backend.calls.length = 0;

    const saving = store.save(item);
    store.follow([]);
    await saving;

    expect(backend.of('memory_held_list')).toEqual([]);
  });

  it('drops a read that was under way when the chat closed, even when it opened again', async () => {
    const { transport, reads } = slowLists();
    const store = new MemoryHeldStore(transport, QUIET);

    store.follow([FIRST]);
    store.follow([]);
    store.follow([FIRST]);
    expect(reads).toHaveLength(2);

    reads[0].answer(sampleHeld(FIRST).slice(0, 1));
    await Promise.resolve();
    await Promise.resolve();
    expect(store.of(FIRST)).toEqual([]);

    reads[1].answer(sampleHeld(FIRST).slice(0, 2));
    await expect.poll(() => store.of(FIRST)).toHaveLength(2);
  });

  it('drops a read that answers after its chat closed', async () => {
    const { transport, reads } = slowLists();
    const store = new MemoryHeldStore(transport, QUIET);

    store.follow([FIRST]);
    store.follow([]);
    reads[0].answer(sampleHeld(FIRST));
    await Promise.resolve();
    await Promise.resolve();

    expect(store.of(FIRST)).toEqual([]);
  });
});

describe('what the user is told about a waiting save', () => {
  const answering: Transport = { invoke: async (name) => (name === 'memory_held_list' ? [] : null) as never };

  function setup(transport: Transport = answering) {
    const push = vi.fn(() => 0);

    return { push, store: new MemoryHeldStore(transport, { push }) };
  }

  const held = (kind: 'note' | 'script') => sampleHeld(FIRST).find((item) => item.kind === kind)!;

  /** A backend that refuses to save the item with this id, or any item without one. */
  function refusingOnly(heldId?: string): Transport {
    return {
      invoke: async (name, args) => {
        if (name === 'memory_held_save' && (heldId === undefined || (args as { heldId: string }).heldId === heldId)) {
          throw new IpcFailure({ code: 'changed', message: 'refused' });
        }

        return [] as never;
      },
    };
  }

  it.each([
    ['note', 'Note saved'],
    ['script', 'Script saved'],
  ] as const)('raises a toast with the title of a saved %s', async (kind, title) => {
    const { store, push } = setup();
    const item = held(kind);

    await store.save(item);

    expect(push).toHaveBeenCalledExactlyOnceWith({ title, description: heldTitle(item), tone: 'success' });
  });

  it.each([
    ['note', 'Note skipped'],
    ['script', 'Script skipped'],
  ] as const)('raises a toast with the title of a skipped %s', async (kind, title) => {
    const { store, push } = setup();
    const item = held(kind);

    await store.skip(item);

    expect(push).toHaveBeenCalledExactlyOnceWith({ title, description: heldTitle(item) });
  });

  it('raises one toast for Save all, counting what was saved, after saving in order', async () => {
    const backend = recordingTransport();
    const push = vi.fn(() => 0);
    const store = new MemoryHeldStore(backend.transport, { push });
    const items = sampleHeld(FIRST);

    await store.saveAll(items);

    expect(backend.of('memory_held_save').map(({ heldId }) => heldId)).toEqual(items.map((item) => item.heldId));
    expect(push).toHaveBeenCalledExactlyOnceWith({ title: 'Saved to memory', description: '3 notes and 1 script', tone: 'success' });
  });

  it('keeps going after a refused save and counts only what was saved', async () => {
    const [first, second, third] = sampleHeld(FIRST).filter((item) => item.kind === 'note');
    const { store, push } = setup(refusingOnly(second.heldId));

    await store.saveAll([first, second, third]);

    expect(push).toHaveBeenCalledExactlyOnceWith({ title: 'Saved to memory', description: '2 notes', tone: 'success' });
    expect(store.failures[second.heldId]).toBe('The agent changed it. Review it again.');
  });

  it('names the item when Save all saved only one, and says nothing when it saved none', async () => {
    const [first, second] = sampleHeld(FIRST).filter((item) => item.kind === 'note');
    const { store, push } = setup(refusingOnly(second.heldId));

    await store.saveAll([second]);
    expect(push).not.toHaveBeenCalled();

    await store.saveAll([first, second]);
    expect(push).toHaveBeenCalledExactlyOnceWith({ title: 'Note saved', description: heldTitle(first), tone: 'success' });
  });

  it('raises none just because waiting saves are listed when a chat opens', async () => {
    const listing: Transport = { invoke: async (name) => (name === 'memory_held_list' ? sampleHeld(FIRST) : null) as never };
    const { store, push } = setup(listing);

    store.follow([FIRST]);
    await store.load();

    expect(store.of(FIRST)).toHaveLength(4);
    expect(push).not.toHaveBeenCalled();
  });

  it('raises none when the save is refused', async () => {
    const { store, push } = setup(refusingOnly());

    await store.save(held('note'));

    expect(push).not.toHaveBeenCalled();
  });
});
