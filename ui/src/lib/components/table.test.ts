import { describe, expect, it } from 'vitest';

import { type Column, nextSort } from './table';

const name: Column = { key: 'name', label: 'Name', sortable: true };
const updated: Column = { key: 'updated', label: 'Updated', sortable: true, descFirst: true };

describe('nextSort', () => {
  it('starts a new column in its natural direction: names from the first, times from the newest', () => {
    expect(nextSort({ key: 'updated', desc: true }, name)).toEqual({ key: 'name', desc: false });
    expect(nextSort({ key: 'name', desc: false }, updated)).toEqual({ key: 'updated', desc: true });
  });

  it('flips the direction of the column that is sorted already', () => {
    expect(nextSort({ key: 'name', desc: false }, name)).toEqual({ key: 'name', desc: true });
    expect(nextSort({ key: 'updated', desc: true }, updated)).toEqual({ key: 'updated', desc: false });
  });
});
