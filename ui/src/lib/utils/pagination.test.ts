import { describe, expect, it } from 'vitest';

import { clampPage, pageCount, pageItems } from './pagination';

describe('pageCount and clampPage', () => {
  it('counts a partial last page and keeps one page for an empty list', () => {
    expect(pageCount(0, 50)).toBe(1);
    expect(pageCount(50, 50)).toBe(1);
    expect(pageCount(51, 50)).toBe(2);
  });

  it('moves a page past the end back to the last one', () => {
    expect(clampPage(9, 120, 50)).toBe(2);
    expect(clampPage(-1, 120, 50)).toBe(0);
    expect(clampPage(1, 0, 50)).toBe(0);
  });
});

describe('pageItems', () => {
  it('shows every page of a short list', () => {
    expect(pageItems(0, 1)).toEqual([0]);
    expect(pageItems(2, 5)).toEqual([0, 1, 2, 3, 4]);
  });

  it('leaves out the middle of a long list with an ellipsis, near either end and in the middle', () => {
    expect(pageItems(0, 20)).toEqual([0, 1, 2, 3, 'gap', 19]);
    expect(pageItems(10, 20)).toEqual([0, 'gap', 9, 10, 11, 'gap', 19]);
    expect(pageItems(19, 20)).toEqual([0, 'gap', 16, 17, 18, 19]);
  });

  it('writes the page itself where an ellipsis would stand for just one', () => {
    expect(pageItems(3, 20)).toEqual([0, 1, 2, 3, 4, 'gap', 19]);
    expect(pageItems(16, 20)).toEqual([0, 'gap', 15, 16, 17, 18, 19]);
  });
});
