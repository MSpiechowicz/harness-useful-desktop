import { describe, expect, it } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import { projectChoice } from '../scope/scope-options';
import { sortDocs } from './doc-groups';
import type { ScopedDoc } from './doc-scope';
import { docTarget, heldIn, newDocTarget, sameTarget, targetRoot } from './doc-target';

const APP: ProjectScope = { path: '/work/app/web', name: 'web', root: '/work/app', key: 'git:github.com/acme/app' };
const SITE: ProjectScope = { path: '/work/site', name: 'site', root: null, key: null };
const PROJECTS = [APP, SITE];

function doc(patch: Partial<ScopedDoc>): ScopedDoc {
  return { origin: 'local', scope: 'project', root: '/work/app', repoKey: null, ...patch };
}

describe('where a new document starts out', () => {
  it('is the project the Scope filter shows, else every project', () => {
    expect(newDocTarget(projectChoice(APP), PROJECTS)).toEqual({ kind: 'repo', path: '/work/app/web' });
    expect(newDocTarget(projectChoice(SITE), PROJECTS)).toEqual({ kind: 'repo', path: '/work/site' });
    expect(newDocTarget('global', PROJECTS)).toEqual({ kind: 'global' });
    expect(newDocTarget('all', PROJECTS)).toEqual({ kind: 'global' });
    expect(newDocTarget('repo:git:github.com/acme/other', PROJECTS)).toEqual({ kind: 'global' });
  });
});

describe('where a document that exists applies', () => {
  it('is the folder of the project with its root, the root itself for a project that left the recent ones, and Global otherwise', () => {
    expect(docTarget(doc({}), PROJECTS)).toEqual({ kind: 'repo', path: '/work/app/web' });
    expect(docTarget(doc({ root: '/old/app' }), PROJECTS)).toEqual({ kind: 'repo', path: '/old/app' });
    expect(docTarget(doc({ scope: 'global', root: null }), PROJECTS)).toEqual({ kind: 'global' });
    expect(docTarget(doc({ root: null }), PROJECTS)).toEqual({ kind: 'global' });
  });
});

describe('the root of a target', () => {
  it('is that of its project, none when the folder is gone or for Global, and the folder itself for a project not listed', () => {
    expect(targetRoot({ kind: 'repo', path: '/work/app/web' }, PROJECTS)).toBe('/work/app');
    expect(targetRoot({ kind: 'repo', path: '/work/site' }, PROJECTS)).toBeNull();
    expect(targetRoot({ kind: 'global' }, PROJECTS)).toBeNull();
    expect(targetRoot({ kind: 'repo', path: '/old/app' }, PROJECTS)).toBe('/old/app');
    expect(sameTarget({ kind: 'global' }, { kind: 'global' })).toBe(true);
    expect(sameTarget({ kind: 'global' }, { kind: 'repo', path: '/work/site' })).toBe(false);
  });
});

describe('how many documents a target holds', () => {
  const docs = [doc({ scope: 'global', root: null }), doc({}), doc({}), doc({ root: '/work/other' }), doc({ origin: 'team' }), doc({ scope: null, root: null })];

  it('counts those of this computer in the scope: the global ones, or the project by its root', () => {
    expect(heldIn(docs, { kind: 'global' }, PROJECTS, (item) => item)).toBe(1);
    expect(heldIn(docs, { kind: 'repo', path: '/work/app/web' }, PROJECTS, (item) => item)).toBe(2);
    expect(heldIn(docs, { kind: 'repo', path: '/work/site' }, PROJECTS, (item) => item)).toBe(0);
  });
});

describe('sortDocs', () => {
  const values = { size: (word: string) => (word === '' ? null : word.length) };

  it('sorts a flat list by a column, the ones with nothing in it last whichever way, and keeps the order for a tie or no sort', () => {
    const words = ['ccc', '', 'a', 'bb', 'dd'];

    expect(sortDocs(words, { key: 'size', desc: false }, values)).toEqual(['a', 'bb', 'dd', 'ccc', '']);
    expect(sortDocs(words, { key: 'size', desc: true }, values)).toEqual(['ccc', 'bb', 'dd', 'a', '']);
    expect(sortDocs(words, null, values)).toBe(words);
    expect(sortDocs(words, { key: 'unknown', desc: false }, values)).toBe(words);
  });
});
