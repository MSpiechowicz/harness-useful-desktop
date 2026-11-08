import { afterEach, describe, expect, it, vi } from 'vitest';

import type { NotesCopyPreview } from '$lib/ipc/bindings/NotesCopyPreview';
import type { NotesCopyRow } from '$lib/ipc/bindings/NotesCopyRow';
import { ipc } from '$lib/ipc/client';
import type { CommandName } from '$lib/ipc/commands';

import { NotesCopyFlow } from './notes-copy-state.svelte';

const row = (title: string, id: number): NotesCopyRow => ({ localId: id, teamId: null, pickable: true, scope: 'global', repo: null, title, action: 'keep', conflict: true, failed: false, failure: null, localUpdatedAt: null, teamUpdatedAt: null });

const PREVIEW = {
  previewId: 'plan-1',
  nothing: null,
  counts: { added: 0, updated: 0, identical: 0, teamNewer: 0, conflicts: 3, overwritten: 0, failed: 0, localDiffers: 0 },
  total: 3,
  rows: [],
} as unknown as NotesCopyPreview;

type Rows = { total: number; rows: NotesCopyRow[] };

/** A flow past its comparison, whose reads of rows the test answers by hand in the order it likes. */
async function flowWithHeldReads() {
  const held: { args: { filter: string; offset: number }; answer: (rows: NotesCopyRow[]) => void }[] = [];

  vi.spyOn(ipc, 'invoke').mockImplementation((async (name: CommandName, args: { filter: string; offset: number }) => {
    if (name === 'memory_notes_copy_preview') {
      return PREVIEW;
    }

    return new Promise<Rows>((resolve) => held.push({ args, answer: (rows) => resolve({ total: rows.length, rows }) }));
  }) as typeof ipc.invoke);

  const flow = new NotesCopyFlow('notes');
  const begun = flow.begin();

  // The first page of the conflicts is the one read the comparison waits for.
  await vi.waitFor(() => expect(held).toHaveLength(1));
  held[0].answer([row('Start', 1)]);
  await begun;
  held.length = 0;

  return { flow, held };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('reading the pages of the copy dialog', () => {
  it('keeps the page the user asked for last when an earlier page answers after it', async () => {
    const { flow, held } = await flowWithHeldReads();

    const slow = flow.go(1);
    const quick = flow.go(2);
    await vi.waitFor(() => expect(held).toHaveLength(2));

    held[1].answer([row('Page two', 51)]);
    await quick;
    expect(flow.rows.map((entry) => entry.title)).toEqual(['Page two']);
    expect(flow.page).toBe(2);
    expect(flow.paging).toBe(false);

    held[0].answer([row('Page one', 1)]);
    await slow;

    expect(flow.rows.map((entry) => entry.title)).toEqual(['Page two']);
    expect(flow.page).toBe(2);
  });

  it('says it is reading until the newest read answers, not until the first one does', async () => {
    const { flow, held } = await flowWithHeldReads();

    const slow = flow.go(1);
    const quick = flow.go(2);
    await vi.waitFor(() => expect(held).toHaveLength(2));

    held[0].answer([row('Page one', 1)]);
    await slow;
    expect(flow.paging).toBe(true);

    held[1].answer([row('Page two', 51)]);
    await quick;
    expect(flow.paging).toBe(false);
  });

  it('shows the rows of the filter chosen last when the one before answers after it', async () => {
    const { flow, held } = await flowWithHeldReads();

    const all = flow.show('all');
    const pickable = flow.show('pickable');
    await vi.waitFor(() => expect(held).toHaveLength(2));
    expect(held.map((read) => read.args.filter)).toEqual(['all', 'pickable']);

    held[1].answer([row('Can be picked', 7)]);
    await pickable;
    held[0].answer([row('Everything', 8)]);
    await all;

    expect(flow.view).toBe('pickable');
    expect(flow.rows.map((entry) => entry.title)).toEqual(['Can be picked']);
  });
});
