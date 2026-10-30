import { describe, expect, it } from 'vitest';

import { railBadge } from './rail-badges';

describe('the rail badge of the Hooks view', () => {
  it('counts the hooks waiting for approval, and is quiet without any', () => {
    expect(railBadge('hooks', null, null, 2)).toEqual({ text: '2', label: '2 hooks to approve', tone: 'warning', busy: false });
    expect(railBadge('hooks', null, null, 1)?.label).toBe('1 hook to approve');
    expect(railBadge('hooks', null, null, 0)).toBeNull();
    expect(railBadge('hooks', null, null)).toBeNull();
  });

  it('caps the count and does not mark any other view', () => {
    expect(railBadge('hooks', null, null, 120)?.text).toBe('99+');
    expect(railBadge('notes', null, null, 3)).toBeNull();
    expect(railBadge('scripts', null, null, 3)).toBeNull();
  });
});

describe('the rail badge of the Hooks view with Harness family updates', () => {
  it('counts the updates in the accent tone when nothing waits for approval', () => {
    expect(railBadge('hooks', null, null, 0, undefined, 1)).toEqual({ text: '1', label: '1 hook update', tone: 'accent', busy: false });
    expect(railBadge('hooks', null, null, 0, undefined, 3)?.label).toBe('3 hook updates');
  });

  it('adds the updates to the hooks to approve, and stays in the warning tone while one waits for approval', () => {
    expect(railBadge('hooks', null, null, 1, undefined, 1)).toEqual({ text: '2', label: '1 hook to approve, 1 hook update', tone: 'warning', busy: false });
    expect(railBadge('hooks', null, null, 2, undefined, 2)?.label).toBe('2 hooks to approve, 2 hook updates');
  });

  it('caps the count and does not mark another view', () => {
    expect(railBadge('hooks', null, null, 90, undefined, 20)?.text).toBe('99+');
    expect(railBadge('notes', null, null, 0, undefined, 2)).toBeNull();
  });
});

describe('the rail badge of the Routines view', () => {
  it('counts the routines to approve together with the runs that ask something', () => {
    expect(railBadge('routines', null, null, 0, { pending: 2, needsYou: 1 })).toEqual({ text: '3', label: '2 routines to approve, 1 routine needs you', tone: 'warning', busy: true });
    expect(railBadge('routines', null, null, 0, { pending: 1, needsYou: 0 })).toEqual({ text: '1', label: '1 routine to approve', tone: 'warning', busy: false });
  });

  it('is quiet with nothing waiting, caps the count and does not mark another view', () => {
    expect(railBadge('routines', null, null)).toBeNull();
    expect(railBadge('routines', null, null, 0, { pending: 150, needsYou: 0 })?.text).toBe('99+');
    expect(railBadge('hooks', null, null, 0, { pending: 2, needsYou: 2 })).toBeNull();
  });
});
