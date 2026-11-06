import { describe, expect, it } from 'vitest';

import type { AuthorRef } from '../bindings/AuthorRef';
import { compareAddedBy } from './authors';

interface Entry {
  id: number;
  addedBy: AuthorRef | null;
}

const by = (id: number, name: string | null): Entry => ({ id, addedBy: name === null ? null : { name, via: null } });

const ENTRIES = [by(1, 'Zoe'), by(2, null), by(3, 'Émile'), by(4, 'emma'), by(5, 'Zoe'), by(6, null), by(7, 'Emile')];

const sorted = (direction: 1 | -1) => [...ENTRIES].sort((a, b) => compareAddedBy(a, b, direction, (entry) => entry.id)).map((entry) => entry.id);

describe('the sort by who added an entry', () => {
  it('goes from A with only ASCII letters folded, ties by id, and the unknown last', () => {
    // Emile, then emma, then Zoe, and Émile after every plain letter, as on the servers.
    expect(sorted(1)).toEqual([7, 4, 1, 5, 3, 2, 6]);
  });

  it('goes from Z with the ties by id the other way, and the unknown still last', () => {
    expect(sorted(-1)).toEqual([3, 5, 1, 4, 7, 6, 2]);
  });
});
