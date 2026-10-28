import { beforeEach, describe, expect, it } from 'vitest';

import { type Column, nextSort, readColumnWidths, saveColumnWidths, widthsForNewColumns } from './table';

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

describe('column widths kept between visits', () => {
  const useful: Column = { key: 'useful', label: 'Useful' };
  const kept = (tableId: string) => JSON.parse(window.localStorage.getItem(`harness:table:${tableId}`) ?? '{}');

  beforeEach(() => window.localStorage.clear());

  it('keeps the width of a column that is not on show when the widths of the others are saved', () => {
    saveColumnWidths('notes', { name: 200, useful: 90 }, [name, useful]);

    // The table opens without the column that needs data, and the user resizes another one.
    saveColumnWidths('notes', { name: 240 }, [name]);

    expect(kept('notes')).toEqual({ name: 240, useful: 90 });
  });

  it('forgets the width of a shown column that was reset', () => {
    saveColumnWidths('notes', { name: 200, useful: 90 }, [name, useful]);
    saveColumnWidths('notes', { useful: 90 }, [name, useful]);

    expect(kept('notes')).toEqual({ useful: 90 });
  });

  it('gives a column that appears later the width kept for it, and leaves the widths on show alone', () => {
    saveColumnWidths('notes', { name: 200, useful: 90 }, [name, useful]);

    expect(readColumnWidths('notes', [name])).toEqual({ name: 200 });
    expect(widthsForNewColumns('notes', [name, useful], { name: 310 })).toEqual({ useful: 90 });
  });
});
