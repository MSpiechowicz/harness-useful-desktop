import { describe, expect, it } from 'vitest';

import { ALL_REPOS, GLOBAL_ONLY, isKeyless, reachArgs, reachKey, reachScope, repoReach } from './memory-reach';

describe('the reach of a Scope choice', () => {
  it('reads every repository, the global entries alone, or one repository by its key', () => {
    expect(reachArgs(ALL_REPOS)).toEqual({ project: null, repos: { kind: 'all' } });
    expect(reachArgs(GLOBAL_ONLY)).toEqual({ project: null });
    expect(reachArgs(repoReach('git:github.com/acme/widget'))).toEqual({ project: null, repos: { kind: 'repo', key: 'git:github.com/acme/widget' } });
    expect(reachScope(ALL_REPOS)).toBeNull();
    expect(reachScope(GLOBAL_ONLY)).toBe('global');
    expect(reachScope(repoReach('path:/work/widget'))).toBe('repo');
  });

  it('has no key for a project Harness Memory knows no repository of', () => {
    const keyless = 'repo:local:/work/scratch';

    expect(reachKey(keyless)).toBeNull();
    expect(reachArgs(keyless)).toEqual({ project: null });
    expect(reachScope(keyless)).toBeNull();
  });

  it('knows a project without a key apart from the choices that read something', () => {
    expect(isKeyless('repo:local:/work/scratch')).toBe(true);
    expect(isKeyless(repoReach('path:/work/widget'))).toBe(false);
    expect(isKeyless(repoReach('git:github.com/acme/widget'))).toBe(false);
    expect(isKeyless(ALL_REPOS)).toBe(false);
    expect(isKeyless(GLOBAL_ONLY)).toBe(false);
  });
});
