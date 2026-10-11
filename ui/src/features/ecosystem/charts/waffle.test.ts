import { describe, expect, it } from 'vitest';

import { waffleCells } from './waffle';

describe('waffleCells', () => {
  it('always hands out exactly 100 squares', () => {
    expect(waffleCells([232, 128, 23, 23, 132]).reduce((sum, count) => sum + count, 0)).toBe(100);
    expect(waffleCells([1, 1, 1]).reduce((sum, count) => sum + count, 0)).toBe(100);
  });

  it('follows the shares by largest remainder', () => {
    expect(waffleCells([232, 128, 23, 23, 132])).toEqual([43, 24, 4, 4, 25]);
  });

  it('keeps a square for a tiny nonzero value and none for zero', () => {
    const cells = waffleCells([9_990, 4, 0]);
    expect(cells[1]).toBe(1);
    expect(cells[2]).toBe(0);
    expect(cells[0]).toBe(99);
  });

  it('draws nothing when there is nothing', () => {
    expect(waffleCells([0, 0])).toEqual([0, 0]);
  });
});
