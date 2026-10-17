import { describe, expect, it } from 'vitest';

import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
import { IpcFailure } from '../transport';
import { memoryNotesHandlers, noteScenarioFromLocation } from './memory-notes-handlers';

const AVAILABLE = () => ({ kind: 'available' }) as const;

function codeOf(call: () => unknown): string | undefined {
  try {
    call();
  } catch (error) {
    return error instanceof IpcFailure ? error.code : undefined;
  }

  return undefined;
}

describe('mock notes', () => {
  it('lists the global notes without a project and the repository ones with it, newest first, and never sends a body', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');

    const without = handlers.memory_notes_list({ project: null, offset: 0 });
    const within = handlers.memory_notes_list({ project: '/work/app', offset: 0 });

    expect(without.notes.every((note) => note.scope === 'global')).toBe(true);
    expect(within.total).toBeGreaterThan(without.total);
    expect(within.notes).toHaveLength(50);
    expect(within.notes.map((note) => Date.parse(note.updatedAt))).toEqual([...within.notes.map((note) => Date.parse(note.updatedAt))].sort((a, b) => b - a));
    expect(JSON.stringify(within)).not.toContain('"body"');
  });

  it('answers a page: the search, the kind, the sort and the window, with the counts of every kind under the search', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const list = (args: Partial<Parameters<typeof handlers.memory_notes_list>[0]>) => handlers.memory_notes_list({ project: '/work/app', offset: 0, ...args });

    const decisions = list({ kind: 'decision' });
    expect(decisions.notes.every((note) => note.kind === 'decision')).toBe(true);
    expect(decisions.total).toBe(decisions.kinds.find((entry) => entry.kind === 'decision')?.count);

    const searched = list({ query: 'dmabuf', kind: 'problem' });
    expect(searched.total).toBe(0);
    expect(searched.kinds.find((entry) => entry.kind === 'fix')?.count).toBe(1);

    const byTitle = list({ sort: 'title', limit: 3 });
    expect(byTitle.notes.map((note) => note.title)).toEqual([...byTitle.notes.map((note) => note.title)].sort((a, b) => a.localeCompare(b)));
    expect(list({ offset: 50, limit: 50 }).notes.length).toBe(list({}).total - 50);
  });

  it('sorts by kind, scope and place, ties by id, and turns each round with the order', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const sorted = (sort: 'kind' | 'scope' | 'place', order: 'asc' | 'desc') => handlers.memory_notes_list({ project: '/work/app', offset: 0, limit: 200, sort, order }).notes;

    const byKind = sorted('kind', 'asc');
    expect(byKind.map((note) => note.kind)).toEqual(byKind.map((note) => note.kind).sort());

    const byScope = sorted('scope', 'asc').map((note) => note.scope);
    expect(byScope.indexOf('repo')).toBeGreaterThan(byScope.lastIndexOf('global'));

    const byPlace = sorted('place', 'asc').map((note) => note.place);
    expect(byPlace.indexOf('team')).toBeGreaterThan(byPlace.lastIndexOf('local'));

    for (const sort of ['kind', 'scope', 'place'] as const) {
      expect(sorted(sort, 'desc').map((note) => note.id)).toEqual(sorted(sort, 'asc').map((note) => note.id).reverse());
    }
  });

  it('gives the body and the links of a note, and refuses one that is not visible', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');

    const detail = handlers.memory_note_get({ id: 4, project: '/work/app' });

    expect(detail.note.body).toContain('DMABUF');
    expect(detail.links.map((link) => [link.id, link.direction])).toEqual([
      [5, 'out'],
      [5, 'in'],
    ]);
    expect(() => handlers.memory_note_get({ id: 4, project: null })).toThrow('note 4 was not found');
  });

  it('deletes a note for good and refuses it afterwards', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');

    handlers.memory_note_delete({ id: 4, project: '/work/app' });

    expect(handlers.memory_notes_list({ project: '/work/app', offset: 0, limit: 200 }).notes.some((note) => note.id === 4)).toBe(false);
    expect(() => handlers.memory_note_get({ id: 4, project: '/work/app' })).toThrow('note 4 was not found');
  });

  it('deletes every note the project sees at once, every page, and counts the ones the team database kept', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const all = (project: string | null) => handlers.memory_notes_list({ project, offset: 0, limit: 200 });

    const global = all(null);
    const repository = all('/work/app').total - global.total;
    expect(global.total).toBeGreaterThan(0);
    expect(repository).toBeGreaterThan(50);

    expect(handlers.memory_notes_delete_all({ project: null })).toEqual({ deleted: global.total, team: global.notes.filter((note) => note.place === 'team').length });
    expect(all(null).total).toBe(0);
    expect(all('/work/app').total).toBe(repository);

    const left = all('/work/app').notes;
    expect(handlers.memory_notes_delete_all({ project: '/work/app' })).toEqual({ deleted: repository, team: left.filter((note) => note.place === 'team').length });
    expect(all('/work/app')).toMatchObject({ total: 0, notes: [] });
    expect(handlers.memory_notes_delete_all({ project: '/work/app' })).toEqual({ deleted: 0, team: 0 });
  });

  it('refuses to delete them all while the server is missing or too old, and deletes nothing', () => {
    let availability: MemoryScriptsAvailability = { kind: 'outdated' };
    const handlers = memoryNotesHandlers(() => availability, () => 'ok');

    expect(codeOf(() => handlers.memory_notes_delete_all({ project: '/work/app' }))).toBe('unsupported');
    availability = { kind: 'notInstalled' };
    expect(codeOf(() => handlers.memory_notes_delete_all({ project: '/work/app' }))).toBe('notFound');

    availability = { kind: 'available' };
    expect(handlers.memory_notes_list({ project: '/work/app', offset: 0 }).total).toBeGreaterThan(0);
  });

  it('reports a missing or old server through the availability, with no notes, and keeps none in the empty scenario', () => {
    const outdated = memoryNotesHandlers(() => ({ kind: 'outdated' }), () => 'ok').memory_notes_list({ project: '/work/app', offset: 0 });
    const empty = memoryNotesHandlers(AVAILABLE, () => 'empty').memory_notes_list({ project: '/work/app', offset: 0 });

    expect(outdated).toMatchObject({ availability: { kind: 'outdated' }, total: 0, notes: [] });
    expect(empty).toMatchObject({ total: 0, notes: [] });
    expect(noteScenarioFromLocation('?notes=empty')).toBe('empty');
    expect(noteScenarioFromLocation('?notes=nope')).toBe('ok');
  });
});
