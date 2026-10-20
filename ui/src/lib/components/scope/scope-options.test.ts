import { describe, expect, it } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import { buildScopeOptions, buildTargetOptions, choiceKey, matchesScope, projectChoice, type ScopeChoice, shortRepoName, targetOf, targetValue } from './scope-options';

const LABELS = { all: 'All', global: 'Global', remoteOnly: 'Remote only' };

const WIDGET: ProjectScope = { path: '/work/widget', name: 'widget', root: '/work/widget', key: 'git:github.com/acme/widget' };
const ALPHA: ProjectScope = { path: '/work/alpha', name: 'Alpha', root: '/work/alpha', key: 'git:github.com/acme/alpha' };
const SCRATCH: ProjectScope = { path: '/tmp/scratch', name: 'scratch', root: null, key: null };

describe('shortRepoName', () => {
  it('drops the host of a repository and leaves a folder as it is', () => {
    expect(shortRepoName('github.com/acme/widget')).toBe('acme/widget');
    expect(shortRepoName('gitlab.example.com/group/sub/project')).toBe('group/sub/project');
    expect(shortRepoName('/home/me/notes')).toBe('/home/me/notes');
    expect(shortRepoName('acme/widget')).toBe('acme/widget');
  });
});

describe('buildScopeOptions', () => {
  it('puts All first, the projects on this computer by name, the remote-only repositories by name, and Global last', () => {
    const options = buildScopeOptions({
      projects: [WIDGET, ALPHA],
      remote: [
        { key: 'git:github.com/zeta/last', label: 'github.com/zeta/last', count: 2 },
        { key: 'git:github.com/acme/widget', label: 'github.com/acme/widget', count: 5 },
        { key: 'git:github.com/beta/first', label: 'github.com/beta/first', count: 1 },
      ],
      globalCount: 3,
      allCount: 11,
      labels: LABELS,
    });

    expect(options.map(({ value, label, hint, count, remoteOnly }) => ({ value, label, hint, count, remoteOnly }))).toEqual([
      { value: 'all', label: 'All', hint: undefined, count: 11, remoteOnly: false },
      { value: 'repo:git:github.com/acme/alpha', label: 'Alpha', hint: 'acme/alpha', count: 0, remoteOnly: false },
      { value: 'repo:git:github.com/acme/widget', label: 'widget', hint: 'acme/widget', count: 5, remoteOnly: false },
      { value: 'repo:git:github.com/beta/first', label: 'beta/first', hint: 'Remote only', count: 1, remoteOnly: true },
      { value: 'repo:git:github.com/zeta/last', label: 'zeta/last', hint: 'Remote only', count: 2, remoteOnly: true },
      { value: 'global', label: 'Global', hint: undefined, count: 3, remoteOnly: false },
    ]);
  });

  it('lists a project with no entries and shows no counts while none are known', () => {
    const known = buildScopeOptions({ projects: [ALPHA], remote: [], labels: LABELS });
    const unknown = buildScopeOptions({ projects: [ALPHA], remote: null, labels: LABELS });

    expect(known[1]).toMatchObject({ label: 'Alpha', count: 0 });
    expect(unknown[1]).toMatchObject({ label: 'Alpha', count: undefined });
  });

  it('flags a project without a repository key and offers it by its folder', () => {
    const [, scratch] = buildScopeOptions({ projects: [SCRATCH], remote: [], labels: LABELS });

    expect(scratch).toMatchObject({ value: 'repo:local:/tmp/scratch', label: 'scratch', keyless: true, path: '/tmp/scratch', root: undefined });
    expect(scratch.hint).toBeUndefined();
  });

  it('takes the count of a project from the counts of the rows when the source has none', () => {
    const counts = new Map<ScopeChoice, number>([[projectChoice(WIDGET), 4]]);
    const options = buildScopeOptions({ projects: [WIDGET, ALPHA], remote: null, counts, labels: LABELS });

    expect(options.map((option) => option.count)).toEqual([undefined, 0, 4, undefined]);
  });

  it('lists two folders of one repository once', () => {
    const worktree: ProjectScope = { ...WIDGET, path: '/work/widget-wt', name: 'widget-wt' };
    const options = buildScopeOptions({ projects: [WIDGET, worktree], remote: [], labels: LABELS });

    expect(options.map((option) => option.label)).toEqual(['All', 'widget', 'Global']);
  });
});

describe('choiceKey and matchesScope', () => {
  const projects = [WIDGET, ALPHA, SCRATCH];

  it('gives the key of a repository choice and none for the others or a project without a key', () => {
    expect(choiceKey(projectChoice(WIDGET))).toBe('git:github.com/acme/widget');
    expect(choiceKey('all')).toBeNull();
    expect(choiceKey('global')).toBeNull();
    expect(choiceKey(projectChoice(SCRATCH))).toBeNull();
  });

  it('shows every row under All and the rows that are not tied to a repository under Global', () => {
    expect(matchesScope('all', { global: false, root: null, key: null }, projects)).toBe(true);
    expect(matchesScope('global', { global: true, root: null, key: null }, projects)).toBe(true);
    expect(matchesScope('global', { global: false, root: '/work/widget', key: null }, projects)).toBe(false);
  });

  it('matches a repository by the key of the row or by the root of the project that has the key', () => {
    const choice = projectChoice(WIDGET);

    expect(matchesScope(choice, { global: false, root: null, key: 'git:github.com/acme/widget' }, projects)).toBe(true);
    expect(matchesScope(choice, { global: false, root: '/work/widget', key: null }, projects)).toBe(true);
    expect(matchesScope(choice, { global: false, root: '/work/alpha', key: 'git:github.com/acme/alpha' }, projects)).toBe(false);
    expect(matchesScope(choice, { global: true, root: '/work/widget', key: 'git:github.com/acme/widget' }, projects)).toBe(false);
    expect(matchesScope(choice, { global: false, root: null, key: null }, projects)).toBe(false);
  });

  it('matches a project without a key by its root alone', () => {
    const withRoot: ProjectScope = { ...SCRATCH, root: '/tmp/scratch' };

    expect(matchesScope(projectChoice(withRoot), { global: false, root: '/tmp/scratch', key: null }, [withRoot])).toBe(true);
    expect(matchesScope(projectChoice(SCRATCH), { global: false, root: '/tmp/scratch', key: null }, projects)).toBe(false);
  });
});

describe('the Applies to options', () => {
  it('offers Global and then the projects on this computer by name with their repository under the name', () => {
    expect(buildTargetOptions([WIDGET, ALPHA, SCRATCH], 'Global')).toEqual([
      { value: 'global', label: 'Global' },
      { value: 'repo:/work/alpha', label: 'Alpha', hint: 'acme/alpha' },
      { value: 'repo:/tmp/scratch', label: 'scratch', hint: undefined },
      { value: 'repo:/work/widget', label: 'widget', hint: 'acme/widget' },
    ]);
  });

  it('lists the scope the field holds when it is no project of the list, by its folder', () => {
    expect(buildTargetOptions([WIDGET], 'Global', { kind: 'repo', path: '/old/gone' }).at(-1)).toEqual({ value: 'repo:/old/gone', label: 'gone', hint: '/old/gone' });
    expect(buildTargetOptions([WIDGET], 'Global', { kind: 'repo', path: '/work/widget' })).toHaveLength(2);
    expect(buildTargetOptions([WIDGET], 'Global', { kind: 'global' })).toHaveLength(2);
  });

  it('turns a target into an option value and back', () => {
    expect(targetOf(targetValue({ kind: 'global' }))).toEqual({ kind: 'global' });
    expect(targetOf(targetValue({ kind: 'repo', path: '/work/widget' }))).toEqual({ kind: 'repo', path: '/work/widget' });
  });
});
