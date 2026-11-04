import { describe, expect, it } from 'vitest';

import { lookOf } from './knowledge-status';

const waiting = { status: 'waiting', pending: null } as const;
const is = (name: string | null | undefined) => (name === undefined || name === null ? null : name.trim().toLowerCase() === 'ada lovelace');
const unknown = () => null;

describe('the badge of a document that waits for approval', () => {
  it('is ready to review for a viewer who is none of its authors, and waiting for one who is', () => {
    expect(lookOf({ ...waiting, authors: ['Grace Hopper'] }, is)).toMatchObject({ label: 'Ready to review', tone: 'warning', outline: false });
    expect(lookOf({ ...waiting, authors: ['Grace Hopper', ' ADA lovelace '] }, is)).toMatchObject({ label: 'Waiting for approval', tone: 'warning', outline: true });
  });

  it('says plainly it waits when the row names no authors or the viewer is not known', () => {
    expect(lookOf({ ...waiting, authors: null }, is)).toMatchObject({ label: 'Waiting for approval', outline: false });
    expect(lookOf({ ...waiting, authors: [] }, is)).toMatchObject({ label: 'Waiting for approval', outline: false });
    expect(lookOf({ ...waiting, authors: ['Grace Hopper'] }, unknown)).toMatchObject({ label: 'Waiting for approval', outline: false });
  });

  it('keeps a live document with a waiting update as an update waiting, and one with changes asked for as that', () => {
    const live = { status: 'live', authors: null } as const;
    const update = { id: 9, revision: 1, updatedAt: '2026-01-01T00:00:00Z' };

    expect(lookOf({ ...live, pending: { ...update, status: 'waiting', authors: ['Grace Hopper'] } }, is)).toMatchObject({ label: 'Update waiting', tone: 'info' });
    expect(lookOf({ ...live, pending: { ...update, status: 'changesRequested' } }, is)).toMatchObject({ label: 'Changes requested', tone: 'danger' });
    expect(lookOf({ ...live, pending: null }, is)).toMatchObject({ label: 'Live', tone: 'success' });
  });
});
