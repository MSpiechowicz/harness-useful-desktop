import { describe, expect, it } from 'vitest';

import { fixtures } from './fixtures';
import { defaultScenario } from './scenarios';
import { createMockTransport } from './transport';

describe('the mock branch sync commands', () => {
  const project = fixtures.recentProjects[0].path;

  it('refuse a project that is neither active nor recent, like the backend', async () => {
    const mock = createMockTransport(defaultScenario(), 0);

    await expect(mock.invoke('git_sync_state', { project: '/etc' })).rejects.toMatchObject({ code: 'forbidden' });
    await expect(mock.invoke('git_fetch', { project: '/etc', background: true })).rejects.toMatchObject({ code: 'forbidden' });
    await expect(
      mock.invoke('git_branch_search', {
        project: '/etc',
        query: '',
        limit: 10,
      }),
    ).rejects.toMatchObject({ code: 'forbidden' });
  });

  it('refuse a branch name git would not take', async () => {
    const mock = createMockTransport(defaultScenario(), 0);

    await expect(
      mock.invoke('git_switch', {
        project,
        request: { branch: 'a..b', create: 'main', stash: false },
      }),
    ).rejects.toMatchObject({ code: 'invalidInput' });
    await expect(mock.invoke('git_set_main_branch', { project, branch: '-x' })).rejects.toMatchObject({ code: 'invalidInput' });
  });

  it('say a folder that is no repository has no branch', async () => {
    const mock = createMockTransport(defaultScenario(), 0);
    const folder = fixtures.recentProjects[1].path;

    await expect(mock.invoke('git_sync_state', { project: folder })).resolves.toMatchObject({ isRepo: false, branch: null });
  });

  it('drop the stash copy only once the stash was applied, and then refuse to apply it', async () => {
    const applied = createMockTransport(defaultScenario(), 0);
    history.replaceState(null, '', '/?sync=stashapplied');

    await expect(applied.invoke('git_sync_action', { project, action: 'applyStash' })).rejects.toMatchObject({ code: 'invalidInput' });
    await expect(applied.invoke('git_sync_action', { project, action: 'dropStash' })).resolves.toMatchObject({
      outcome: 'done',
      state: { stashKept: false, stashApplied: false },
    });
    await expect(applied.invoke('git_sync_action', { project, action: 'dropStash' })).rejects.toMatchObject({ code: 'invalidInput' });

    history.replaceState(null, '', '/?sync=stash');
    const waiting = createMockTransport(defaultScenario(), 0);

    await expect(waiting.invoke('git_sync_action', { project, action: 'dropStash' })).rejects.toMatchObject({ code: 'invalidInput' });
    history.replaceState(null, '', '/');
  });
});
