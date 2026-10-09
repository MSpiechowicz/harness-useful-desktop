import { describe, expect, it } from 'vitest';

import { fuzzyMatch, highlightSegments } from './fuzzy';

describe('fuzzyMatch', () => {
  it('matches everything for an empty query', () => {
    expect(fuzzyMatch('  ', 'anything')).toEqual({ score: 0, indices: [] });
  });

  it('returns null when characters are missing or out of order', () => {
    expect(fuzzyMatch('xyz', 'Go to Chat')).toBeNull();
    expect(fuzzyMatch('tg', 'go')).toBeNull();
  });

  it('ranks a contiguous word-start match above a scattered one', () => {
    const contiguous = fuzzyMatch('set', 'Go to Settings');
    const scattered = fuzzyMatch('set', 'Show extensions tab');

    expect(contiguous && scattered && contiguous.score > scattered.score).toBe(true);
  });

  it('reports matched indices case-insensitively', () => {
    expect(fuzzyMatch('GTC', 'Go to Chat')?.indices).toEqual([0, 3, 6]);
  });
});

describe('highlightSegments', () => {
  it('merges adjacent characters into runs', () => {
    expect(highlightSegments('Settings', [0, 1, 4])).toEqual([
      { text: 'Se', match: true },
      { text: 'tt', match: false },
      { text: 'i', match: true },
      { text: 'ngs', match: false },
    ]);
  });
});
