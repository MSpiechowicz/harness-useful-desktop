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
    expect(railBadge('memory', null, null, 3)).toBeNull();
  });
});
