import { describe, expect, it } from 'vitest';

import { diffFields, reviewChanges } from './doc-diff';
import { MAX_PER_SCOPE, scopeIsFull } from './doc-groups';
import { emptyReason, emptySearchOnEscape } from './doc-view';
import { reviewTitleKey, type ReviewTitleKeys } from './review-title';

describe('the scope limit', () => {
  it('knows when a scope is full, and does not count a document that stays in it twice', () => {
    expect(scopeIsFull(MAX_PER_SCOPE, false)).toBe(true);
    expect(scopeIsFull(MAX_PER_SCOPE, true)).toBe(false);
    expect(scopeIsFull(MAX_PER_SCOPE - 1, false)).toBe(false);
  });
});

describe('diffFields', () => {
  type Field = 'name' | 'text' | 'note';
  const order: readonly Field[] = ['name', 'text', 'note'];

  it('lists the parts that differ, in order, with the empty ones left out of a new document', () => {
    expect(diffFields(order, { name: 'A', text: 'x', note: null }, { name: 'A', text: 'y', note: null })).toEqual([{ field: 'text', from: 'y', to: 'x' }]);
    expect(diffFields(order, { name: 'A', text: 'x', note: null }, null)).toEqual([
      { field: 'name', from: null, to: 'A' },
      { field: 'text', from: null, to: 'x' },
    ]);
    expect(diffFields(order, { name: 'A', text: 'x', note: 'n' }, { name: 'A', text: 'x', note: 'n' })).toEqual([]);
  });

  it('words the changes for a reader', () => {
    const changes = diffFields(order, { name: 'A', text: 'x', note: null }, null);

    expect(reviewChanges(changes, { label: (field) => field.toUpperCase(), value: (_field, value) => (value === null ? null : `<${value}>`), text: 'text', mono: new Set(['name']) })).toEqual([
      { key: 'name', label: 'NAME', text: false, from: null, to: '<A>', mono: true },
      { key: 'text', label: 'TEXT', text: true, from: null, to: '<x>', mono: false },
    ]);
  });
});

describe('the headline of a review', () => {
  const keys: ReviewTitleKeys = {
    added: 'routines.review.added',
    addedOutside: 'routines.review.addedOutside',
    changed: 'routines.review.changed',
    changedOutside: 'routines.review.changedOutside',
    teamAdded: 'routines.review.team.added',
    teamChanged: 'routines.review.team.changed',
  };

  it('says who added or changed the document, and a team document whose it is', () => {
    expect(reviewTitleKey(keys, { team: false, added: true, byAgent: true })).toBe(keys.added);
    expect(reviewTitleKey(keys, { team: false, added: true, byAgent: false })).toBe(keys.addedOutside);
    expect(reviewTitleKey(keys, { team: false, added: false, byAgent: true })).toBe(keys.changed);
    expect(reviewTitleKey(keys, { team: false, added: false, byAgent: false })).toBe(keys.changedOutside);
    expect(reviewTitleKey(keys, { team: true, added: true, byAgent: true })).toBe(keys.teamAdded);
    expect(reviewTitleKey(keys, { team: true, added: false, byAgent: false })).toBe(keys.teamChanged);
  });
});

describe('the view helpers', () => {
  it('says why a list shows no row: the search found nothing, or the filters leave nothing', () => {
    expect(emptyReason(false, 0, '', false)).toBeNull();
    expect(emptyReason(true, 3, 'x', true)).toBeNull();
    expect(emptyReason(true, 0, '', true)).toBe('filter');
    expect(emptyReason(true, 0, 'x', true)).toBe('search');
    expect(emptyReason(true, 0, '  ', false)).toBe('search');
  });

  it('lets Escape empty a filled search, and leaves an empty one for the panel to close', () => {
    let query = 'lint';
    const event = (key: string) => {
      const keyboard = new KeyboardEvent('keydown', { key, cancelable: true });

      emptySearchOnEscape(keyboard, query, () => (query = ''));

      return keyboard.defaultPrevented;
    };

    expect(event('a')).toBe(false);
    expect(event('Escape')).toBe(true);
    expect(query).toBe('');
    expect(event('Escape')).toBe(false);
  });
});
