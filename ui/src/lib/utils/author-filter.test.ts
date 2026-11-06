import { describe, expect, it } from 'vitest';

import type { AuthorFacet } from '$lib/ipc/bindings/AuthorFacet';
import type { AuthorFilter } from '$lib/ipc/bindings/AuthorFilter';

import { AUTHOR_FACET_CAP, authorFromValue, authorMenuShown, authorOptions, authorValue } from './author-filter';

const LABELS = { all: 'All', unknown: 'Unknown' };

const FACET: AuthorFacet = {
  people: [
    { name: 'Ada Lovelace', count: 5 },
    { name: 'Grace Hopper', count: 2 },
  ],
  unknown: 3,
};

describe('the menu value of an author filter', () => {
  it('reads back what it wrote, for everyone, the unknown and a person', () => {
    const filters: (AuthorFilter | null)[] = [null, { kind: 'unknown' }, { kind: 'name', name: 'Ada Lovelace' }];

    for (const filter of filters) {
      expect(authorFromValue(authorValue(filter))).toEqual(filter);
    }
  });

  it('keeps a person called all or unknown apart from those choices', () => {
    expect(authorValue({ kind: 'name', name: 'all' })).not.toBe(authorValue(null));
    expect(authorValue({ kind: 'name', name: 'unknown' })).not.toBe(authorValue({ kind: 'unknown' }));

    expect(authorFromValue(authorValue({ kind: 'name', name: 'all' }))).toEqual({ kind: 'name', name: 'all' });
    expect(authorFromValue(authorValue({ kind: 'name', name: 'unknown' }))).toEqual({ kind: 'name', name: 'unknown' });
  });

  it('keeps a name with a colon whole', () => {
    expect(authorFromValue(authorValue({ kind: 'name', name: 'name:a:b' }))).toEqual({ kind: 'name', name: 'name:a:b' });
  });
});

describe('the choices of the Added by menu', () => {
  it('lists All with the total, each person in the server order, and Unknown with its count', () => {
    const options = authorOptions(FACET, null, LABELS);

    expect(options.map((option) => [option.label, option.count])).toEqual([
      ['All', 10],
      ['Ada Lovelace', 5],
      ['Grace Hopper', 2],
      ['Unknown', 3],
    ]);
  });

  it('has no Unknown while every entry has a name, unless it is the chosen one', () => {
    const named: AuthorFacet = { people: FACET.people, unknown: 0 };

    expect(authorOptions(named, null, LABELS).map((option) => option.label)).toEqual(['All', 'Ada Lovelace', 'Grace Hopper']);
    expect(authorOptions(named, { kind: 'unknown' }, LABELS).map((option) => [option.label, option.count])).toContainEqual(['Unknown', 0]);
  });

  it('keeps the chosen person among the choices when the other filters leave nothing of them', () => {
    const options = authorOptions({ people: [{ name: 'Ada Lovelace', count: 1 }], unknown: 0 }, { kind: 'name', name: 'Grace Hopper' }, LABELS);

    expect(options.map((option) => [option.label, option.count])).toEqual([
      ['All', undefined],
      ['Ada Lovelace', 1],
      ['Grace Hopper', 0],
    ]);
    expect(options.some((option) => option.value === authorValue({ kind: 'name', name: 'Grace Hopper' }))).toBe(true);
  });

  it('keeps the All count while the facet is complete and names the chosen person', () => {
    expect(authorOptions(FACET, { kind: 'name', name: 'Ada Lovelace' }, LABELS)[0].count).toBe(10);
    expect(authorOptions(FACET, { kind: 'unknown' }, LABELS)[0].count).toBe(10);
  });

  it('leaves out the All count when the facet may have left people out, and says nothing of a chosen person it may have left out', () => {
    const people = Array.from({ length: AUTHOR_FACET_CAP }, (_, index) => ({ name: `Person ${index}`, count: 2 }));
    const full: AuthorFacet = { people, unknown: 1 };
    const almost: AuthorFacet = { people: people.slice(1), unknown: 1 };

    expect(authorOptions(full, null, LABELS)[0].count).toBeUndefined();
    expect(authorOptions(almost, null, LABELS)[0].count).toBe(2 * (AUTHOR_FACET_CAP - 1) + 1);

    const beyond = authorOptions(full, { kind: 'name', name: 'Person 999' }, LABELS);

    expect(beyond[0].count).toBeUndefined();
    expect(beyond.find((option) => option.label === 'Person 999')?.count).toBeUndefined();
  });

  it('leaves out the All count for a chosen person the facet does not name', () => {
    expect(authorOptions(FACET, { kind: 'name', name: 'Grace Hopper X' }, LABELS)[0].count).toBeUndefined();
  });

  it('keeps the chosen person among the choices while the facet is not there', () => {
    const options = authorOptions(null, { kind: 'name', name: 'Grace Hopper' }, LABELS);

    expect(options.map((option) => option.label)).toEqual(['All', 'Grace Hopper']);
    expect(options.every((option) => option.count === undefined)).toBe(true);
  });
});

describe('when the Added by menu is shown', () => {
  const nobody: AuthorFacet = { people: [], unknown: 4 };

  it('is shown when somebody can be chosen', () => {
    expect(authorMenuShown(true, FACET, false)).toBe(true);
  });

  it('stays while a search or a filter narrows the list, though nobody is left to choose', () => {
    expect(authorMenuShown(true, nobody, true)).toBe(true);
    expect(authorMenuShown(true, { people: [], unknown: 0 }, true)).toBe(true);
  });

  it('is hidden when every entry is unknown and nothing narrows the list, and in a project with nothing', () => {
    expect(authorMenuShown(true, nobody, false)).toBe(false);
    expect(authorMenuShown(true, { people: [], unknown: 0 }, false)).toBe(false);
  });

  it('is hidden for a server that cannot filter by author', () => {
    expect(authorMenuShown(false, FACET, true)).toBe(false);
    expect(authorMenuShown(false, null, true)).toBe(false);
  });
});
