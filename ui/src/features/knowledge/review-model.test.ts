import { describe, expect, it } from 'vitest';

import type { KnowledgeHistoryEvent } from '$lib/ipc/bindings/KnowledgeHistoryEvent';

import { mineOf, reviewModel, type IsViewer, type Version } from './review-model';

const NOW = Date.now();
const at = (minutes: number): string => new Date(NOW - minutes * 60_000).toISOString();

/** Ada drafts, Bob renames, Bob asks: the events come newest first. */
const EVENTS: KnowledgeHistoryEvent[] = [
  { kind: 'asked', by: 'Bob Stone', at: at(1), comment: null },
  { kind: 'edited', by: 'Bob Stone', at: at(2), comment: null },
  { kind: 'drafted', by: 'Ada Lovelace', at: at(3), comment: null },
];

const viewer =
  (name: string | null): IsViewer =>
  (person) =>
    name === null || !person?.trim() ? null : name.toLowerCase() === person.trim().toLowerCase();

function version(extra: Partial<Version> = {}): Version {
  // The last writer is Bob (a rename), the server's list holds both of them.
  return { id: 7, status: 'waiting', revision: 3, update: false, author: 'Bob Stone', updatedAt: at(2), ...extra };
}

function review(who: string, events: KnowledgeHistoryEvent[], extra: Partial<Version> = {}) {
  const v = version(extra);

  return reviewModel({ version: v, approval: true, mine: mineOf(v, events, viewer(who)), readOnly: false, noName: false, pin: null, events, lastChange: null });
}

const steps = (who: string, events: KnowledgeHistoryEvent[], extra: Partial<Version> = {}): string[] => review(who, events, extra).actions.map((action) => action.step);

describe.each([
  ['with the list of authors the server prints', { authors: ['Ada Lovelace', 'Bob Stone'] }, EVENTS],
  ['without it, from the history', {}, EVENTS],
])('a renamed draft, %s', (_name, extra, events) => {
  it.each(['Ada Lovelace', 'Bob Stone', 'bob stone'])('shows %s the author view: Take it back, no Approve', (who) => {
    expect(steps(who, events, extra)).toEqual(['takeBack']);
  });

  it('shows a third teammate Approve and Request changes', () => {
    expect(steps('Carol Shaw', events, extra)).toEqual(['approve', 'requestChanges']);
  });
});

describe('who counts as a writer', () => {
  it('lets the server list decide: a name it does not list is not the author, whatever the history says', () => {
    expect(mineOf(version({ authors: ['Bob Stone'] }), EVENTS, viewer('Ada Lovelace'))).toBe(false);
  });

  it('matches the list trimmed and without case, like the server', () => {
    expect(mineOf(version({ authors: ['  ADA lovelace '] }), [], viewer('Ada Lovelace'))).toBe(true);
  });

  it('ignores writers before the version last went live', () => {
    const events: KnowledgeHistoryEvent[] = [{ kind: 'asked', by: 'Bob Stone', at: at(1), comment: null }, { kind: 'wentLive', by: 'Dan Poe', at: at(5), comment: null }, { kind: 'drafted', by: 'Ada Lovelace', at: at(9), comment: null }];

    expect(mineOf(version(), events, viewer('Ada Lovelace'))).toBe(false);
    expect(mineOf(version(), events, viewer('Bob Stone'))).toBe(true);
  });

  it('only counts the asker of a version that waits', () => {
    const events: KnowledgeHistoryEvent[] = [{ kind: 'asked', by: 'Eve Ray', at: at(1), comment: null }];

    expect(mineOf(version({ status: 'draft' }), events, viewer('Eve Ray'))).toBe(false);
    expect(mineOf(version({ status: 'waiting' }), events, viewer('Eve Ray'))).toBe(true);
  });

  it('is not known while the name of the user is not, unless they are named', () => {
    expect(mineOf(version({ authors: ['Ada Lovelace'] }), [], viewer(null))).toBeNull();
    expect(mineOf(version(), EVENTS, viewer(null))).toBeNull();
  });
});
