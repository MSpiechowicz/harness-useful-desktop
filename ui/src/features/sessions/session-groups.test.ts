import { describe, expect, it } from 'vitest';

import type { SessionSummary } from '$lib/ipc/bindings/SessionSummary';

import { forkCount, groupSessions } from './session-groups';

function session(id: string, overrides: Partial<SessionSummary> = {}): SessionSummary {
  return { ref: { harness: 'omp', id }, parentSession: null, emptyCopy: false, ...overrides } as SessionSummary;
}

const ids = (list: SessionSummary[]) => list.map((summary) => summary.ref.id);

describe('session groups', () => {
  it('folds empty copies under the session they copy and keeps the other order', () => {
    const { rows, copiesByRoot, emptyCopies } = groupSessions([
      session('copy-b', { parentSession: 'root', emptyCopy: true }),
      session('fork', { parentSession: 'root' }),
      session('root'),
      session('copy-a', { parentSession: 'root', emptyCopy: true }),
      session('other'),
    ]);

    expect(ids(rows)).toEqual(['fork', 'root', 'other']);
    expect(ids(copiesByRoot.get('root') ?? [])).toEqual(['copy-b', 'copy-a']);
    expect(copiesByRoot.has('fork')).toBe(false);
    expect(ids(emptyCopies)).toEqual(['copy-b', 'copy-a']);
  });

  it('folds a copy of a copy under the session at the end of the chain', () => {
    const { rows, copiesByRoot } = groupSessions([
      session('second', { parentSession: 'first', emptyCopy: true }),
      session('first', { parentSession: 'root', emptyCopy: true }),
      session('root'),
    ]);

    expect(ids(rows)).toEqual(['root']);
    expect(ids(copiesByRoot.get('root') ?? [])).toEqual(['second', 'first']);
  });

  it('keeps a copy whose original is not in the list as a row, still counted for clean-up', () => {
    const { rows, copiesByRoot, emptyCopies } = groupSessions([session('orphan', { parentSession: 'gone', emptyCopy: true }), session('root')]);

    expect(ids(rows)).toEqual(['orphan', 'root']);
    expect(copiesByRoot.size).toBe(0);
    expect(ids(emptyCopies)).toEqual(['orphan']);
  });

  it('does not loop on copies that point at each other', () => {
    const { rows, emptyCopies } = groupSessions([session('a', { parentSession: 'b', emptyCopy: true }), session('b', { parentSession: 'a', emptyCopy: true })]);

    expect(ids(rows)).toEqual(['a', 'b']);
    expect(emptyCopies).toHaveLength(2);
  });
});

describe('fork count', () => {
  it('counts the sessions forked from one, not its empty copies', () => {
    const sessions = [session('fork-a', { parentSession: 'root' }), session('fork-b', { parentSession: 'root' }), session('copy', { parentSession: 'root', emptyCopy: true }), session('root')];

    expect(forkCount(sessions, 'root')).toBe(2);
    expect(forkCount(sessions, 'fork-a')).toBe(0);
  });
});
