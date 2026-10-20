import { afterEach, describe, expect, it } from 'vitest';

import { fixtures } from './fixtures';
import { DEMO_REPO, REMOTE_ONLY_REPO } from './project-scopes';
import { defaultScenario } from './scenarios';
import { createMockTransport } from './transport';

const transport = () => createMockTransport(defaultScenario(), 0);
const [demo, research, legacy] = fixtures.recentProjects;

/** Opens the mock with `search` as the address query. */
function preview(search: string): ReturnType<typeof transport> {
  globalThis.history.replaceState(null, '', `/${search}`);

  return transport();
}

afterEach(() => globalThis.history.replaceState(null, '', '/'));

describe('project_scopes', () => {
  it('lists the active project first, then the recent ones once, with their roots and repository keys', async () => {
    const mock = transport();
    await mock.invoke('project_open', { path: research.path });

    await expect(mock.invoke('project_scopes', undefined)).resolves.toEqual([
      { path: research.path, name: research.name, root: research.path, key: `path:${research.path}` },
      { path: demo.path, name: demo.name, root: demo.path, key: DEMO_REPO },
      { path: legacy.path, name: legacy.name, root: null, key: null },
    ]);
  });

  it('shares the demo project key with the sample notes repository, and the research folder has none of its notes', async () => {
    const mock = transport();
    const { repos } = await mock.invoke('memory_repos_list', { kind: 'notes' });
    const scopes = await mock.invoke('project_scopes', undefined);

    expect(repos.map((repo) => repo.key)).toContain(DEMO_REPO);
    expect(repos.map((repo) => repo.key)).not.toContain(scopes.find((scope) => scope.path === research.path)?.key);
  });

  it('has no keys while Harness Memory is not installed', async () => {
    const scopes = await preview('?scripts=notInstalled').invoke('project_scopes', undefined);

    expect(scopes.map((scope) => scope.key)).toEqual([null, null, null]);
    expect(scopes[0].root).toBe(demo.path);
  });

  it('forgets a project with the recent list', async () => {
    const mock = transport();
    await mock.invoke('project_forget', { path: research.path });

    await expect(mock.invoke('project_scopes', undefined)).resolves.toHaveLength(2);
  });

  it('clears the recent projects when remembering is turned off', async () => {
    const mock = transport();
    const prefs = await mock.invoke('prefs_get', undefined);

    await mock.invoke('prefs_set', { prefs: { ...prefs, rememberRecentProjects: false } });

    await expect(mock.invoke('project_recent', undefined)).resolves.toEqual([]);
    await expect(mock.invoke('project_scopes', undefined)).resolves.toEqual([]);
  });
});

describe('listing every hook and routine', () => {
  it('adds the team hooks of a repository no project here has, by key only, and the hooks of every project', async () => {
    const mock = preview('?hooks=team');
    await mock.invoke('hook_save', {
      draft: { id: null, expectedSha256: null, name: 'Research notes', description: null, scope: 'project', project: research.path, event: 'sessionStart', matcher: null, action: { type: 'addContext', text: 'Notes.' }, enabled: true },
    });

    const scoped = await mock.invoke('hooks_list', { project: demo.path });
    const all = await mock.invoke('hooks_list', { project: demo.path, all: true });

    expect(scoped.hooks.some((hook) => hook.repoKey === REMOTE_ONLY_REPO || hook.root === research.path)).toBe(false);
    expect(all.hooks.find((hook) => hook.repoKey === REMOTE_ONLY_REPO)).toMatchObject({ origin: 'team', root: null });
    expect(all.hooks.find((hook) => hook.root === research.path)).toMatchObject({ origin: 'local', repoKey: `path:${research.path}` });
    expect(all.hooks.filter((hook) => hook.origin === 'team' && hook.repoKey === DEMO_REPO).every((hook) => hook.root === demo.path)).toBe(true);
    expect(all.hooks.length).toBe(scoped.hooks.length + 2);
  });

  it('adds the team routines of a repository no project here has, by key only', async () => {
    const mock = preview('?routines=team');

    const scoped = await mock.invoke('routines_list', { project: demo.path });
    const all = await mock.invoke('routines_list', { project: demo.path, all: true });

    expect(scoped.routines.some((routine) => routine.repoKey === REMOTE_ONLY_REPO)).toBe(false);
    expect(all.routines.find((routine) => routine.repoKey === REMOTE_ONLY_REPO)).toMatchObject({ origin: 'team', root: null });
    expect(all.routines.find((routine) => routine.origin === 'team' && routine.repoKey === DEMO_REPO)).toMatchObject({ root: demo.path });
    expect(all.counts.total).toBe(scoped.counts.total + 1);
  });
});
