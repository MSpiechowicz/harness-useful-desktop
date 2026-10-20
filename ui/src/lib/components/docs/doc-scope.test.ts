import { describe, expect, it } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import { deleteScopeText, docScope, docScopeOptions, inScope, openProjectOf, scopeSortValue, type ScopedDoc, scopeText, versionScopeText } from './doc-scope';

const LABELS = { all: 'All', global: 'Global', remoteOnly: 'Remote only' };

const PROJECTS: ProjectScope[] = [
  { path: '/work/widget', name: 'widget', root: '/work/widget', key: 'git:github.com/acme/widget' },
  { path: '/work/notes', name: 'notes', root: '/work/notes', key: null },
];

function doc(patch: Partial<ScopedDoc>): ScopedDoc {
  return { origin: 'local', scope: 'project', root: null, repoKey: null, ...patch };
}

const DOCS: ScopedDoc[] = [
  doc({ scope: 'global' }),
  doc({ root: '/work/widget' }),
  doc({ origin: 'team', repoKey: 'git:github.com/acme/widget' }),
  doc({ root: '/work/notes' }),
  doc({ origin: 'team', repoKey: 'git:github.com/acme/site' }),
  doc({ scope: null }),
];

describe('docScope', () => {
  it('names the project on this computer, else the repository or folder, and says when the scope is unknown', () => {
    expect(docScope(DOCS[0], PROJECTS)).toEqual({ kind: 'global' });
    expect(docScope(DOCS[1], PROJECTS)).toEqual({ kind: 'repo', label: 'widget' });
    expect(docScope(DOCS[2], PROJECTS)).toEqual({ kind: 'repo', label: 'widget' });
    expect(docScope(DOCS[4], PROJECTS)).toEqual({ kind: 'repo', label: 'acme/site' });
    expect(docScope(doc({ root: '/old/app' }), PROJECTS)).toEqual({ kind: 'repo', label: 'app' });
    expect(docScope(DOCS[5], PROJECTS)).toEqual({ kind: 'unknown' });
    expect(docScope(doc({}), PROJECTS)).toEqual({ kind: 'unknown' });
  });
});

describe('docScopeOptions', () => {
  it('counts the rows of each scope and lists the repositories only team rows name', () => {
    const options = docScopeOptions(DOCS, PROJECTS, LABELS);

    expect(options.map((option) => [option.label, option.count, option.remoteOnly])).toEqual([
      ['All', 6, false],
      ['notes', 1, false],
      ['widget', 2, false],
      ['acme/site', 1, true],
      ['Global', 1, false],
    ]);
  });

  it('lists a project with no rows too', () => {
    const options = docScopeOptions([DOCS[0]], PROJECTS, LABELS);

    expect(options.find((option) => option.label === 'widget')?.count).toBe(0);
  });
});

describe('inScope', () => {
  it('keeps a project by its key or its root, the global rows for Global, and every row for All', () => {
    const options = docScopeOptions(DOCS, PROJECTS, LABELS);
    const widget = options.find((option) => option.label === 'widget')!.value;
    const notes = options.find((option) => option.label === 'notes')!.value;
    const site = options.find((option) => option.label === 'acme/site')!.value;
    const id = (view: ScopedDoc) => view;

    expect(inScope(DOCS, 'all', id, PROJECTS)).toHaveLength(6);
    expect(inScope(DOCS, 'global', id, PROJECTS)).toEqual([DOCS[0]]);
    expect(inScope(DOCS, widget, id, PROJECTS)).toEqual([DOCS[1], DOCS[2]]);
    expect(inScope(DOCS, notes, id, PROJECTS)).toEqual([DOCS[3]]);
    expect(inScope(DOCS, site, id, PROJECTS)).toEqual([DOCS[4]]);
  });
});

describe('scopeText and scopeSortValue', () => {
  it('say a scope in a sentence and sort Global first, the repositories by name and an unknown scope last', () => {
    const scopes = DOCS.map((candidate) => docScope(candidate, PROJECTS));

    expect(scopes.map(scopeText)).toEqual(['Global', 'widget', 'widget', 'notes', 'acme/site', 'Unknown']);
    expect(scopes.map(scopeSortValue)).toEqual(['', 'widget', 'widget', 'notes', 'acme/site', null]);
  });
});

describe('versionScopeText', () => {
  it('names the project a version applies to, which is another one when it moved, and Global when it was widened', () => {
    const row = doc({ root: '/work/widget', repoKey: 'git:github.com/acme/widget' });

    expect(versionScopeText(row, { scope: 'project', project: '/work/widget' }, PROJECTS)).toBe('widget');
    expect(versionScopeText(row, { scope: 'project', project: '/work/notes' }, PROJECTS)).toBe('notes');
    expect(versionScopeText(row, { scope: 'project', project: '/elsewhere/app' }, PROJECTS)).toBe('app');
    expect(versionScopeText(row, { scope: 'global', project: null }, PROJECTS)).toBe('Global');
  });
});

describe('deleteScopeText', () => {
  it('says Global and the open project, or only Global while no project is open', () => {
    expect(deleteScopeText(openProjectOf(PROJECTS, '/work/notes'))).toBe('Global and notes');
    expect(deleteScopeText(openProjectOf(PROJECTS, null))).toBe('Global');
    expect(deleteScopeText(openProjectOf(PROJECTS, '/gone'))).toBe('Global');
  });
});
