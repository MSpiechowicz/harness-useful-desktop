import { describe, expect, it } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import { deleteReach, deleteScopeText, docScope, docScopeOptions, memoryScope, inScope, resolveScope, scopeSortValue, type ScopedDoc, scopeText, versionScopeText } from './doc-scope';

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
    expect(deleteScopeText('notes')).toBe('Global and notes');
    expect(deleteScopeText(null)).toBe('Global');
  });
});

describe('deleteReach', () => {
  it('names the open project as the delete takes it and gives the root its documents are under', () => {
    expect(deleteReach({ path: '/work/notes', name: 'notes' }, PROJECTS)).toEqual({ project: 'notes', root: '/work/notes', known: true });
  });

  it('reaches only the global documents with no open project', () => {
    expect(deleteReach(null, PROJECTS)).toEqual({ project: null, root: null, known: true });
  });

  it('still names the open project, but is not known, while the projects are not read or lack it or its folder is gone', () => {
    const open = { path: '/work/notes', name: 'notes' };

    expect(deleteReach(open, [])).toEqual({ project: 'notes', root: null, known: false });
    expect(deleteReach(open, [PROJECTS[0]])).toEqual({ project: 'notes', root: null, known: false });
    expect(deleteReach(open, [{ ...PROJECTS[1], root: null }])).toEqual({ project: 'notes', root: null, known: false });
  });
});

describe('resolveScope', () => {
  const options = docScopeOptions(DOCS, PROJECTS, LABELS);

  it('keeps a choice the filter lists, or any choice while what it lists is not known', () => {
    expect(resolveScope('repo:git:github.com/acme/widget', options, true)).toBe('repo:git:github.com/acme/widget');
    expect(resolveScope('global', options, true)).toBe('global');
    expect(resolveScope('repo:git:github.com/gone/app', options, false)).toBe('repo:git:github.com/gone/app');
  });

  it('counts a choice the filter no longer lists as All', () => {
    expect(resolveScope('repo:git:github.com/gone/app', options, true)).toBe('all');
  });
});

describe('memoryScope', () => {
  it('names a note or script by its project on this computer, else by owner/repo or folder', () => {
    expect(memoryScope({ scope: 'global', repo: null }, PROJECTS, 'This project')).toEqual({ kind: 'global' });
    expect(memoryScope({ scope: 'repo', repo: 'github.com/acme/widget' }, PROJECTS, 'This project')).toEqual({ kind: 'repo', label: 'widget' });
    expect(memoryScope({ scope: 'repo', repo: 'github.com/acme/site' }, PROJECTS, 'This project')).toEqual({ kind: 'repo', label: 'acme/site' });
    expect(memoryScope({ scope: 'repo', repo: 'path:/work/old-app' }, PROJECTS, 'This project')).toEqual({ kind: 'repo', label: 'old-app' });
    expect(memoryScope({ scope: 'repo', repo: null }, PROJECTS, 'This project')).toEqual({ kind: 'repo', label: 'This project' });
  });
});
