import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

const invoke = vi.hoisted(() => vi.fn());

vi.mock('$lib/ipc/client', () => ({ ipc: { invoke } }));

import { project } from './project.svelte';
import { projectScopes, revalidateProjectScopes } from './project-scopes.svelte';

const WIDGET: ProjectScope = { path: '/work/widget', name: 'widget', root: '/work/widget', key: 'git:github.com/acme/widget' };

describe('the projects of the Scope filters', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue([WIDGET]);
    projectScopes.sync(null, null);
    project.active = null;
    project.recents = [];
  });

  it('reads the projects when a view opens and again, quietly, at the next opening', async () => {
    revalidateProjectScopes();
    await projectScopes.refresh();

    expect(projectScopes.data).toEqual([WIDGET]);
    expect(projectScopes.status).toBe('ready');
    expect(invoke).toHaveBeenCalledWith('project_scopes', undefined);

    invoke.mockClear();
    revalidateProjectScopes();
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
  });

  it('reads them again from the start when the open project changes', async () => {
    revalidateProjectScopes();
    await projectScopes.refresh();
    invoke.mockClear();
    invoke.mockResolvedValue([]);

    project.active = { path: '/work/widget', name: 'widget', exists: true, isGitRepo: true };
    revalidateProjectScopes();

    expect(projectScopes.status).toBe('loading');
    await vi.waitFor(() => expect(projectScopes.data).toEqual([]));
  });
});
