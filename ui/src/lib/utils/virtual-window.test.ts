import { describe, expect, it } from 'vitest';

import { computeOffsets, computeWindow } from './virtual-window';

describe('computeWindow', () => {
  const fixed = computeOffsets(1000, () => 20);

  it('renders only the rows in view plus overscan', () => {
    const window = computeWindow(fixed, 400, 100, 2);

    expect(window).toEqual({ start: 18, end: 28, total: 20_000 });
  });

  it('clamps to the list bounds at the start and end', () => {
    expect(computeWindow(fixed, 0, 100, 3).start).toBe(0);
    expect(computeWindow(fixed, 19_950, 100, 3).end).toBe(1000);
  });

  it('handles variable row heights', () => {
    const heights = [10, 50, 10, 100, 10];
    const offsets = computeOffsets(heights.length, (index) => heights[index]);

    expect(offsets).toEqual([0, 10, 60, 70, 170, 180]);
    expect(computeWindow(offsets, 65, 10, 0)).toEqual({ start: 2, end: 4, total: 180 });
  });

  it('returns an empty window for an empty list', () => {
    expect(computeWindow(computeOffsets(0, () => 20), 0, 100, 5)).toEqual({ start: 0, end: 0, total: 0 });
  });
});
