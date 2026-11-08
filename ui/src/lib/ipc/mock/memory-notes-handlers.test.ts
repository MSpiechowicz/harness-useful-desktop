import { describe, expect, it } from 'vitest';

import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
import { IpcFailure } from '../transport';
import { combinesFilters, memoryNotesHandlers, noteScenarioFromLocation } from './memory-notes-handlers';

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

  it('combines the scope with the kind: kinds are counted under the scope, scopes under the kind, and the total follows both', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const list = (args: Partial<Parameters<typeof handlers.memory_notes_list>[0]>) => handlers.memory_notes_list({ project: '/work/app', offset: 0, ...args });
    const kindCount = (page: ReturnType<typeof list>, kind: string) => page.kinds.find((entry) => entry.kind === kind)?.count;

    const everything = list({});
    expect(everything).toMatchObject({ combinedFilters: true, total: 70, scopes: { repo: 55, global: 15 } });

    const global = list({ scope: 'global' });
    expect(global.total).toBe(15);
    expect(global.notes.every((note) => note.scope === 'global')).toBe(true);
    expect(global.kinds.reduce((sum, entry) => sum + entry.count, 0)).toBe(15);
    expect(global.scopes).toEqual({ repo: 55, global: 15 });

    const decisions = list({ kind: 'decision' });
    expect(decisions.scopes).toEqual({ repo: 8, global: 3 });
    expect(kindCount(decisions, 'fix')).toBe(10);

    const both = list({ scope: 'repo', kind: 'decision' });
    expect(both.total).toBe(8);
    expect(both.notes.every((note) => note.scope === 'repo' && note.kind === 'decision')).toBe(true);
    expect(both.scopes).toEqual({ repo: 8, global: 3 });
    expect(kindCount(both, 'decision')).toBe(8);
    expect(kindCount(both, 'fix')).toBe(8);
  });

  it('answers like an older server on request: the scope is dropped and nothing is counted by scope', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok', () => false);

    const page = handlers.memory_notes_list({ project: '/work/app', offset: 0, scope: 'global', kind: 'decision' });

    expect(page).toMatchObject({ combinedFilters: false, scopes: null, total: 11 });
    expect(page.kinds.find((entry) => entry.kind === 'fix')?.count).toBe(10);
    expect(combinesFilters('?filters=old')).toBe(false);
    expect(combinesFilters('?notes=empty')).toBe(true);
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

    expect(handlers.memory_notes_delete_all({ project: null })).toEqual({ deleted: global.total, team: global.notes.filter((note) => note.place === 'team').length, kept: 0 });
    expect(all(null).total).toBe(0);
    expect(all('/work/app').total).toBe(repository);

    const left = all('/work/app').notes;
    expect(handlers.memory_notes_delete_all({ project: '/work/app' })).toEqual({ deleted: repository, team: left.filter((note) => note.place === 'team').length, kept: 0 });
    expect(all('/work/app')).toMatchObject({ total: 0, notes: [] });
    expect(handlers.memory_notes_delete_all({ project: '/work/app' })).toEqual({ deleted: 0, team: 0, kept: 0 });
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

describe('mock notes on the team side', () => {
  it('hides a team note on this computer and lists it only as hidden', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const first = handlers.memory_notes_list({ project: '/work/app', offset: 0, limit: 200 });

    expect(first.hiddenCount).toBe(0);

    handlers.memory_note_hide({ id: 3, project: '/work/app', hidden: true });

    const after = handlers.memory_notes_list({ project: '/work/app', offset: 0, limit: 200 });
    expect(after.total).toBe(first.total - 1);
    expect(after.hiddenCount).toBe(1);
    expect(handlers.memory_notes_list({ project: '/work/app', offset: 0, hidden: true }).notes.map((note) => note.id)).toEqual([3]);
    expect(codeOf(() => handlers.memory_note_get({ id: 3, project: '/work/app' }))).toBe('notFound');
    expect(handlers.memory_note_get({ id: 3, project: '/work/app', hidden: true }).note.hidden).toBe(true);

    handlers.memory_note_hide({ id: 3, project: '/work/app', hidden: false });

    expect(handlers.memory_notes_list({ project: '/work/app', offset: 0, limit: 200 }).total).toBe(first.total);
  });

  it('refuses to hide a note saved on this computer', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');

    expect(codeOf(() => handlers.memory_note_hide({ id: 4, project: '/work/app', hidden: true }))).toBe('process');
  });

  it('keeps the read-only team notes when everything is deleted, and counts them', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'readonly');
    const team = handlers.memory_notes_list({ project: null, offset: 0, limit: 200 }).notes.filter((note) => note.place === 'team').length;

    expect(handlers.memory_notes_delete_all({ project: null })).toMatchObject({ team: 0, kept: team });
    expect(handlers.memory_notes_list({ project: null, offset: 0 }).total).toBe(team);
  });

  it('copies by the review, overwrites only the conflicts picked and keeps the rest on this computer', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const local = handlers.memory_notes_list({ project: '/work/app', offset: 0 }).localCounts;
    const preview = handlers.memory_notes_copy_preview({ kind: 'notes' });

    expect(preview.total).toBe((local?.repo ?? 0) + (local?.global ?? 0));
    expect(preview.counts.conflicts).toBeGreaterThan(1);

    const conflicts = handlers.memory_notes_copy_rows({ kind: 'notes', previewId: preview.previewId, filter: 'conflicts', offset: 0, limit: 200 });
    expect(conflicts.total).toBe(preview.counts.conflicts);
    expect(conflicts.rows.every((row) => row.conflict)).toBe(true);

    expect(codeOf(() => handlers.memory_notes_copy_apply({ kind: 'notes', previewId: preview.previewId, overwrite: [-1] }))).toBe('invalidInput');

    const result = handlers.memory_notes_copy_apply({ kind: 'notes', previewId: preview.previewId, overwrite: [conflicts.rows[0].localId ?? -1] });
    expect(result).toMatchObject({ counts: { overwritten: 1 }, failed: [] });
    expect(result.kept).toBe(preview.counts.conflicts - 1 + preview.counts.teamNewer);
    expect(codeOf(() => handlers.memory_notes_copy_apply({ kind: 'notes', previewId: preview.previewId, overwrite: 'none' }))).toBe('invalidInput');
  });

  it('copies to the remote only the ticked rows, nothing ticked at first, and keeps the rest on this computer', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const local = handlers.memory_notes_list({ project: '/work/app', offset: 0 }).localCounts;
    const preview = handlers.memory_notes_copy_preview({ kind: 'notes', mode: 'toRemote' });
    const pickable = handlers.memory_notes_copy_rows({ kind: 'notes', mode: 'toRemote', previewId: preview.previewId, filter: 'pickable', offset: 0, limit: 200 });

    expect(pickable.total).toBe(preview.counts.added + preview.counts.conflicts);
    expect(pickable.total).toBeLessThan(preview.total);
    expect(preview.rows.filter((row) => !row.pickable).map((row) => row.action)).toEqual(expect.arrayContaining(['identical', 'teamNewer']));

    const identical = preview.rows.find((row) => row.action === 'identical');
    expect(codeOf(() => handlers.memory_notes_copy_apply({ kind: 'notes', mode: 'toRemote', previewId: preview.previewId }))).toBe('invalidInput');
    expect(codeOf(() => handlers.memory_notes_copy_apply({ kind: 'notes', mode: 'toRemote', previewId: preview.previewId, pick: 'none' }))).toBe('invalidInput');
    expect(codeOf(() => handlers.memory_notes_copy_apply({ kind: 'notes', mode: 'toRemote', previewId: preview.previewId, pick: [identical?.localId ?? -1] }))).toBe('invalidInput');
    expect(codeOf(() => handlers.memory_notes_copy_apply({ kind: 'notes', mode: 'toRemote', previewId: preview.previewId, pick: 'all', overwrite: 'all' }))).toBe('invalidInput');

    const ticked = pickable.rows.slice(0, 2);
    const result = handlers.memory_notes_copy_apply({ kind: 'notes', mode: 'toRemote', previewId: preview.previewId, pick: ticked.map((row) => row.localId ?? -1) });

    expect(result).toMatchObject({ deleted: 2, kept: 0, leftover: [], failed: [], counts: { overwritten: ticked.filter((row) => row.conflict).length } });
    expect(handlers.memory_notes_list({ project: '/work/app', offset: 0 }).localCounts).toEqual({
      repo: (local?.repo ?? 0) - ticked.filter((row) => row.scope === 'repo').length,
      global: (local?.global ?? 0) - ticked.filter((row) => row.scope === 'global').length,
    });
  });

  it('copies to this computer by the ticked team ids, from a review of its own, and changes nothing there', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const stranded = handlers.memory_notes_copy_preview({ kind: 'notes' });
    const preview = handlers.memory_notes_copy_preview({ kind: 'notes', mode: 'toLocal' });
    const pickable = handlers.memory_notes_copy_rows({ kind: 'notes', mode: 'toLocal', previewId: preview.previewId, filter: 'pickable', offset: 0, limit: 200 });

    expect(preview.counts).toMatchObject({ added: pickable.total, conflicts: 0 });
    expect(preview.counts.localDiffers).toBeGreaterThan(0);
    expect(preview.rows.filter((row) => !row.pickable).map((row) => row.action).sort()).toEqual(expect.arrayContaining(['identical', 'localDiffers']));
    expect(codeOf(() => handlers.memory_notes_copy_rows({ kind: 'notes', mode: 'toLocal', previewId: stranded.previewId, offset: 0, limit: 50 }))).toBe('invalidInput');
    expect(codeOf(() => handlers.memory_notes_copy_apply({ kind: 'notes', mode: 'toLocal', previewId: preview.previewId }))).toBe('invalidInput');
    expect(codeOf(() => handlers.memory_notes_copy_apply({ kind: 'notes', mode: 'toLocal', previewId: preview.previewId, pick: [-1] }))).toBe('invalidInput');

    const ticked = pickable.rows.slice(0, 2).map((row) => row.teamId ?? -1);
    const result = handlers.memory_notes_copy_apply({ kind: 'notes', mode: 'toLocal', previewId: preview.previewId, pick: ticked });

    expect(result).toMatchObject({ counts: { added: 2 }, deleted: 0, kept: 0, leftover: [], failed: [] });
    expect(handlers.memory_notes_copy_rows({ kind: 'notes', previewId: stranded.previewId, offset: 0, limit: 1 }).total).toBe(stranded.total);
  });

  it('deletes what stayed on this computer', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const local = handlers.memory_notes_list({ project: '/work/app', offset: 0 }).localCounts;

    expect(handlers.memory_notes_discard_local({ ids: 'all' })).toBe((local?.repo ?? 0) + (local?.global ?? 0));
    expect(handlers.memory_notes_list({ project: '/work/app', offset: 0 }).localCounts).toEqual({ repo: 0, global: 0 });
    expect(codeOf(() => handlers.memory_notes_discard_local({ ids: 'none' }))).toBe('invalidInput');
  });

  it('filters by the language the server detected, and says nothing about it for a server without notes-lang', () => {
    const asked = { project: '/work/app', offset: 0 } as const;
    const listed = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const old = memoryNotesHandlers(AVAILABLE, () => 'ok', undefined, undefined, undefined, () => false);

    const flagged = listed.memory_notes_list({ ...asked, lang: 'other' });

    expect(flagged.total).toBeGreaterThan(0);
    expect(flagged.notes.every((note) => note.lang === 'other')).toBe(true);
    expect(listed.memory_notes_list({ ...asked, lang: 'unknown' }).total).toBe(0);

    const ignored = old.memory_notes_list({ ...asked, lang: 'other' });

    expect(ignored.total).toBe(listed.memory_notes_list(asked).total);
    expect(ignored.notes.every((note) => note.lang === null)).toBe(true);
  });

  it('counts how useful notes were: filters by it, sorts by it, and says nothing for a server without notes-useful', () => {
    const asked = { project: '/work/app', offset: 0, limit: 200 } as const;
    const listed = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const old = memoryNotesHandlers(AVAILABLE, () => 'ok', undefined, undefined, undefined, undefined, () => false);

    const ever = listed.memory_notes_list({ ...asked, useful: 'ever' });
    const never = listed.memory_notes_list({ ...asked, useful: 'never' });

    expect(ever.total).toBeGreaterThan(0);
    expect(ever.notes.every((note) => (note.useful ?? 0) > 0)).toBe(true);
    expect(never.total).toBeGreaterThan(0);
    expect(never.notes.every((note) => note.useful === 0 && (note.shown ?? 0) >= 3)).toBe(true);
    expect(ever.total + never.total).toBeLessThan(listed.memory_notes_list(asked).total);

    const byUseful = listed.memory_notes_list({ ...asked, sort: 'useful', order: 'desc' }).notes.map((note) => note.useful ?? 0);
    expect(byUseful).toEqual([...byUseful].sort((a, b) => b - a));

    const byTime = listed.memory_notes_list({ ...asked, sort: 'lastUseful', order: 'desc' }).notes.map((note) => (note.lastUsefulAt ? Date.parse(note.lastUsefulAt) : 0));
    expect(byTime).toEqual([...byTime].sort((a, b) => b - a));

    const ignored = old.memory_notes_list({ ...asked, useful: 'ever', sort: 'useful' });

    expect(ignored.total).toBe(listed.memory_notes_list(asked).total);
    expect(ignored.notes.every((note) => note.shown === null && note.useful === null && note.lastUsefulAt === null)).toBe(true);
  });

  it('says who added and changed a note, and nothing for a server without notes-authors', () => {
    const listed = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const old = memoryNotesHandlers(AVAILABLE, () => 'ok', combinesFilters, undefined, undefined, undefined, undefined, () => false);
    const asked = { project: '/work/app', offset: 0, limit: 200 };

    const notes = listed.memory_notes_list(asked).notes;
    expect(notes.some((note) => note.addedBy?.name === 'Ada Lovelace' && note.addedBy.via === 'claude-code')).toBe(true);
    expect(notes.some((note) => note.updatedBy?.name === 'Grace Hopper' && note.updatedBy.via === null)).toBe(true);
    expect(notes.some((note) => note.addedBy === null && note.updatedBy === null)).toBe(true);
    expect(listed.memory_note_get({ id: notes[0].id, project: '/work/app' }).note.updatedBy).toEqual(notes[0].updatedBy);

    expect(old.memory_notes_list(asked).notes.every((note) => !('addedBy' in note) && !('updatedBy' in note))).toBe(true);
    expect('addedBy' in old.memory_note_get({ id: notes[0].id, project: '/work/app' }).note).toBe(false);
  });

  it('filters by author and counts the authors under every other filter, not the author one', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');
    const list = (args: Partial<Parameters<typeof handlers.memory_notes_list>[0]>) => handlers.memory_notes_list({ project: '/work/app', offset: 0, limit: 200, ...args });

    const everyone = list({ kind: 'decision' });
    const grace = list({ kind: 'decision', author: { kind: 'name', name: 'Grace Hopper' } });
    const nobody = list({ kind: 'decision', author: { kind: 'unknown' } });

    expect(everyone.authors?.people.map((person) => person.name)).toContain('Grace Hopper');
    expect(grace.notes.length).toBeGreaterThan(0);
    expect(grace.notes.every((note) => note.kind === 'decision' && note.addedBy?.name === 'Grace Hopper')).toBe(true);
    expect(nobody.notes.every((note) => note.addedBy === null)).toBe(true);

    // Whoever is chosen, the choices stay as the other filters leave them.
    expect(grace.authors).toEqual(everyone.authors);
    expect(nobody.authors).toEqual(everyone.authors);
    expect(list({ kind: 'fix' }).authors).not.toEqual(everyone.authors);

  });

  it('sorts by author from A or from Z, with the notes nobody added last and the ties by id the way the sort goes', () => {
    const handlers = memoryNotesHandlers(AVAILABLE, () => 'ok');

    for (const order of ['asc', 'desc'] as const) {
      const rows = handlers.memory_notes_list({ project: '/work/app', offset: 0, limit: 200, sort: 'addedBy', order }).notes;
      const named = rows.filter((note) => note.addedBy);
      const unknown = rows.filter((note) => !note.addedBy);
      const sign = order === 'asc' ? 1 : -1;

      expect(unknown.length).toBeGreaterThan(0);
      expect(rows.slice(named.length)).toEqual(unknown);
      expect(new Set(named.map((note) => note.addedBy?.name)).size).toBeGreaterThan(1);

      const nameOf = (note: (typeof rows)[number]) => note.addedBy?.name.toLowerCase() ?? '';
      const expected = [...named].sort((a, b) => sign * ((nameOf(a) < nameOf(b) ? -1 : nameOf(a) > nameOf(b) ? 1 : 0) || a.id - b.id));

      expect(named.map((note) => note.id)).toEqual(expected.map((note) => note.id));
      expect(unknown.map((note) => note.id)).toEqual([...unknown.map((note) => note.id)].sort((a, b) => sign * (a - b)));
    }
  });

  it('answers no facet and ignores the author and its sort for a server that cannot filter by author', () => {
    const old = memoryNotesHandlers(AVAILABLE, () => 'ok', undefined, undefined, undefined, undefined, undefined, undefined, () => false);
    const page = old.memory_notes_list({ project: '/work/app', offset: 0, limit: 200, author: { kind: 'unknown' }, sort: 'addedBy' });

    expect(page.authors).toBeNull();
    expect(page.notes.some((note) => note.addedBy !== null)).toBe(true);
    expect(page.notes.map((note) => Date.parse(note.updatedAt))).toEqual([...page.notes.map((note) => Date.parse(note.updatedAt))].sort((a, b) => b - a));
  });
});
