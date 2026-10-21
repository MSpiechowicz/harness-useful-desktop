import { describe, expect, it } from 'vitest';

import type { HeldItem } from '$lib/ipc/bindings/HeldItem';
import { HELD_NOTE_UPDATE_ID, sampleHeld } from '$lib/ipc/mock/memory-guards-fixtures';

import { heldView } from './memory-held';

const KEY = '11111111-2222-4333-8444-555555555555';

/** The sample note update, with its arguments replaced. */
function update(args: Record<string, unknown>): HeldItem {
  const sample = sampleHeld(KEY).find((item) => item.heldId === HELD_NOTE_UPDATE_ID)!;

  return { ...sample, arguments: { id: 3, ...args } as HeldItem['arguments'] };
}

function note(item: HeldItem) {
  const view = heldView(item);

  if (view.type !== 'note') {
    throw new Error('a note was expected');
  }

  return view;
}

describe('the note a held update would leave', () => {
  it('is the saved body with the appended text after a line break', () => {
    const view = note(update({ append: 'Check it in WebKitGTK.' }));

    expect(view.note.body).toBe('WebKitGTK falls back to a 60 fps timer here.\nCheck it in WebKitGTK.');
    expect(view.current?.body).toBe('WebKitGTK falls back to a 60 fps timer here.');
  });

  it('appends to the body the update sends when it sends one', () => {
    const view = note(update({ body: 'A new body.', append: '  And more.\r\n' }));

    expect(view.note.body).toBe('A new body.\nAnd more.');
  });

  it('adds nothing for an empty append', () => {
    expect(note(update({ append: '  ' })).note.body).toBe('WebKitGTK falls back to a 60 fps timer here.');
  });
});

describe('words the server already normalised', () => {
  it('are compared trimmed and lowercase anyway', () => {
    const view = note(update({ kind: ' Fix ', scope: ' GLOBAL ' }));

    expect(view.note.kind).toBe('fix');
    expect(view.note.scope).toBe('global');
  });

  it('read a script language the same way', () => {
    const script = sampleHeld(KEY).find((item) => item.kind === 'script')!;
    const view = heldView({ ...script, arguments: { ...(script.arguments as object), language: ' Python ', scope: 'Repo ' } as HeldItem['arguments'] });

    expect(view.type === 'script' && [view.script.language, view.script.scope]).toEqual(['python', 'repo']);
  });
});
