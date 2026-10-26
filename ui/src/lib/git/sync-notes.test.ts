import { describe, expect, it } from 'vitest';

import { fixtures } from '$lib/ipc/mock/fixtures';

import { noteFor, noteParts } from './sync-notes';

describe('the note after a sync action', () => {
  const state = structuredClone(fixtures.git.sync);

  it('warns, and does not claim success, when the stash would not go back cleanly', () => {
    for (const action of ['rebase', 'merge', 'applyStash'] as const) {
      expect(noteFor(action, 'stashKept', state, state)).toEqual({
        tone: 'warning',
        text: 'Your changes couldn’t be put back cleanly. They are kept in the stash.',
      });
    }
  });

  it('reports a finished action as success', () => {
    expect(noteFor('rebase', 'done', state, state)?.tone).toBe('success');
  });

  it('reports a dropped stash copy as success', () => {
    expect(noteFor('dropStash', 'done', state, state)).toEqual({
      tone: 'success',
      text: 'Copy dropped',
    });
  });

  it('counts what a push sent', () => {
    expect(noteFor('push', 'done', { ...state, ahead: 2 }, state)?.detail).toBe('2 commits sent');
  });
});

describe('the branch names inside a note', () => {
  it('are cut out of the words around them, the longest name first', () => {
    expect(noteParts('Created feat/x.y from feat', ['feat', 'feat/x.y'])).toEqual([
      { text: 'Created ', name: false },
      { text: 'feat/x.y', name: true },
      { text: ' from ', name: false },
      { text: 'feat', name: true },
    ]);
  });

  it('leave a note without names whole', () => {
    expect(noteParts('Already up to date')).toEqual([{ text: 'Already up to date', name: false }]);
  });
});
