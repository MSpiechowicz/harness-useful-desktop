import { describe, expect, it } from 'vitest';

import { HELD_NOTE_SAVE_ID, HELD_NOTE_UPDATE_ID, sampleHeld } from '$lib/ipc/mock/memory-guards-fixtures';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { recordingTransport } from '../../../tests/chat-helpers';
import { MemoryHeldStore } from './memoryHeld.svelte';

const FIRST = '11111111-2222-4333-8444-555555555555';
const SECOND = '66666666-7777-4888-8999-aaaaaaaaaaaa';

describe('the waiting saves, per chat', () => {
  it('lists each open chat by its own key and stops reading a chat that closed', async () => {
    const backend = recordingTransport();
    const store = new MemoryHeldStore(backend.transport);

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
    const store = new MemoryHeldStore(backend.transport);
    const item = sampleHeld(FIRST).find((held) => held.heldId === HELD_NOTE_SAVE_ID)!;
    store.items = [item];

    await store.save(item);

    expect(backend.of('memory_held_save')).toEqual([{ heldId: HELD_NOTE_SAVE_ID, edited: null, revision: item.revision }]);
  });

  it('keeps the item, says the agent changed it and reads the chat’s list again', async () => {
    const older = sampleHeld(FIRST).find((held) => held.heldId === HELD_NOTE_UPDATE_ID)!;
    const newer = { ...older, revision: 'f'.repeat(64), arguments: { ...(older.arguments as object), body: 'The agent’s newer body.' } } as typeof older;
    const backend = refusing('changed', () => [newer]);
    const store = new MemoryHeldStore(backend.transport);
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
    const store = new MemoryHeldStore(backend.transport);
    const item = sampleHeld(FIRST).find((held) => held.heldId === HELD_NOTE_SAVE_ID)!;
    store.items = [item];

    await store.save({ ...item, revision: 'f'.repeat(64) }, { title: 'Edited' }, item.revision);

    expect(backend.of('memory_held_save')).toEqual([{ heldId: HELD_NOTE_SAVE_ID, edited: { title: 'Edited' }, revision: item.revision }]);
  });
});

describe('skipping a waiting save', () => {
  it('sends the revision it showed, so the backend drops only that', async () => {
    const backend = recordingTransport();
    const store = new MemoryHeldStore(backend.transport);
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
    const store = new MemoryHeldStore(transport);
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
    const store = new MemoryHeldStore(backend.transport);
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
    const store = new MemoryHeldStore(transport);

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
    const store = new MemoryHeldStore(transport);

    store.follow([FIRST]);
    store.follow([]);
    reads[0].answer(sampleHeld(FIRST));
    await Promise.resolve();
    await Promise.resolve();

    expect(store.of(FIRST)).toEqual([]);
  });
});
