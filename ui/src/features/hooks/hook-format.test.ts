import { describe, expect, it } from 'vitest';

import type { Hook } from '$lib/ipc/bindings/Hook';
import type { HookStatus } from '$lib/ipc/bindings/HookStatus';
import type { HookView } from '$lib/ipc/bindings/HookView';
import { LOCAL_ORIGIN } from '$lib/ipc/mock/hooks-team-state';

import { deletableCount, filterCounts, hookDoc, inSource, inStatus, inWhen, matchesQuery, pendingCount, viewKey, waitingFor } from './hook-format';

const LOCAL = { ...LOCAL_ORIGIN, shareBlock: null } as const;

function hook(patch: Partial<Hook> & Pick<Hook, 'id' | 'name'>): Hook {
  return {
    description: null,
    scope: 'global',
    project: null,
    event: 'sessionStart',
    matcher: null,
    action: { type: 'addContext', text: 'Look at the notes.' },
    enabled: true,
    requires: [],
    seed: null,
    createdAt: '2026-10-17T10:00:00Z',
    createdBy: 'desktop',
    updatedAt: '2026-10-17T10:00:00Z',
    updatedBy: 'desktop',
    ...patch,
  };
}

function view(patch: Partial<Hook> & Pick<Hook, 'id' | 'name'>, status: HookStatus = 'active'): HookView {
  return { id: patch.id, hook: hook(patch), name: patch.name, status, sha256: 'a'.repeat(64), problem: null, approval: status === 'pending' ? 'never' : 'approved', approved: null, ...LOCAL };
}

const broken: HookView = { id: 'f'.repeat(32), hook: null, name: 'Broken', status: 'invalid', sha256: 'b'.repeat(64), problem: 'bad', approval: 'never', approved: null, ...LOCAL };

const LIST = [
  view({ id: '1', name: 'Search first', event: 'sessionStart' }),
  view({ id: '2', name: 'Save notes', event: 'turnEnd', action: { type: 'followUp', text: 'Save what you learned.' } }, 'paused'),
  view({ id: '3', name: 'Run lint', scope: 'project', project: '/work/app', event: 'toolEnd', matcher: 'edit|write', action: { type: 'command', command: 'bun run lint', timeoutSecs: null } }, 'pending'),
  view({ id: '4', name: 'Remind about tests', scope: 'project', project: '/work/app', event: 'promptSubmit', enabled: false }, 'off'),
  broken,
];

describe('the counts of the filters', () => {
  it('count every hook by event and by status', () => {
    expect(filterCounts(LIST)).toEqual({
      when: { all: 5, sessionStart: 1, promptSubmit: 1, toolEnd: 1, turnEnd: 1, sessionEnd: 0 },
      status: { all: 5, active: 1, pending: 1, off: 1, paused: 1, invalid: 1, duplicate: 0 },
      source: { all: 5, mine: 5, team: 0 },
    });
  });

  it('count the hooks that wait for approval', () => {
    expect(pendingCount({ hooks: LIST, folderProblem: null, team: null })).toBe(1);
    expect(pendingCount(null)).toBe(0);
    expect(pendingCount({ hooks: [], folderProblem: null, team: null })).toBe(0);
  });
});

describe('the filters', () => {
  const byWhen = (when: Parameters<typeof inWhen>[1]) => LIST.filter((item) => inWhen(item, when)).map((item) => item.name);
  const byStatus = (status: Parameters<typeof inStatus>[1]) => LIST.filter((item) => inStatus(item, status)).map((item) => item.name);

  it('narrow the list to an event', () => {
    expect(byWhen('all')).toHaveLength(5);
    expect(byWhen('toolEnd')).toEqual(['Run lint']);
    expect(byWhen('sessionEnd')).toEqual([]);
  });

  it('narrow the list to a status', () => {
    expect(byStatus('all')).toHaveLength(5);
    expect(byStatus('active')).toEqual(['Search first']);
    expect(byStatus('pending')).toEqual(['Run lint']);
    expect(byStatus('off')).toEqual(['Remind about tests']);
    expect(byStatus('paused')).toEqual(['Save notes']);
    expect(byStatus('invalid')).toEqual(['Broken']);
  });

  it('leave out the files that break the format when an event is chosen', () => {
    expect(byWhen('sessionStart')).not.toContain('Broken');
  });

  it('search the name, description, tool filter, text and command', () => {
    const search = (query: string) => LIST.filter((item) => matchesQuery(item, query)).map((item) => item.name);

    expect(search('')).toHaveLength(5);
    expect(search('LINT')).toEqual(['Run lint']);
    expect(search('write')).toEqual(['Run lint']);
    expect(search('learned')).toEqual(['Save notes']);
    expect(search('bad')).toEqual(['Broken']);
    expect(search('zzz')).toEqual([]);
  });
});

describe('where a hook applies', () => {
  it('is the scope of its file with the project it names, and unknown for a file that is not a hook', () => {
    const located = { ...LIST[2], root: '/work/app', repoKey: 'git:github.com/acme/app' };

    expect(hookDoc(LIST[0])).toEqual({ origin: 'local', scope: 'global', root: null, repoKey: null });
    expect(hookDoc(located)).toEqual({ origin: 'local', scope: 'project', root: '/work/app', repoKey: 'git:github.com/acme/app' });
    expect(hookDoc(broken).scope).toBeNull();
  });
});

describe('the hooks Delete all removes', () => {
  const OTHER = { ...view({ id: '5', name: 'Elsewhere', scope: 'project', project: '/work/other' }), root: '/work/other' };
  const MINE = { ...LIST[3], root: '/work/app' };

  it('are the global ones, the files that are not hooks and the open project\'s', () => {
    expect(deletableCount([LIST[0], MINE, OTHER, broken], '/work/app')).toBe(3);
    expect(deletableCount([LIST[0], MINE, OTHER, broken], null)).toBe(2);
  });
});

describe('the team hooks', () => {
  const team = (id: string, name: string, status: HookStatus = 'active'): HookView => ({ ...view({ id, name }, status), origin: 'team', teamAuthor: 'Anna', teamUpdatedAt: '2026-10-17T10:00:00Z', teamRepo: null });
  const WITH_TEAM = [...LIST, team('1', 'Shared', 'duplicate'), team('9', 'Use commits')];

  it('are told apart from a local hook with the same id by their row key', () => {
    expect(viewKey(WITH_TEAM[0])).toBe('1');
    expect(viewKey(WITH_TEAM[5])).toBe('team:1');
    expect(new Set(WITH_TEAM.map(viewKey)).size).toBe(WITH_TEAM.length);
  });

  it('narrow the list to the user\'s own hooks or the team\'s', () => {
    expect(WITH_TEAM.filter((item) => inSource(item, 'team')).map((item) => item.name)).toEqual(['Shared', 'Use commits']);
    expect(WITH_TEAM.filter((item) => inSource(item, 'mine'))).toHaveLength(5);
    expect(WITH_TEAM.filter((item) => inSource(item, 'all'))).toHaveLength(7);
  });

  it('are counted by source and by the overridden status', () => {
    expect(filterCounts(WITH_TEAM)).toMatchObject({ source: { all: 7, mine: 5, team: 2 }, status: { duplicate: 1, active: 2 } });
  });

  it('are never counted among the hooks Delete all removes', () => {
    expect(deletableCount(WITH_TEAM, '/work/app')).toBe(LIST.length);
  });
});

describe('what a paused hook waits for', () => {
  it('is Harness Memory, off or not installed, and Harness Dashboard', () => {
    expect(waitingFor(hook({ id: '1', name: 'A', requires: ['harness-memory'] }), false)).toEqual({ member: 'memory', off: true });
    expect(waitingFor(hook({ id: '1', name: 'A', requires: ['harness-memory'] }), true)).toEqual({ member: 'memory', off: false });
    expect(waitingFor(hook({ id: '1', name: 'A', requires: ['harness-dashboard'] }), false)).toEqual({ member: 'dashboard', off: false });
    expect(waitingFor(hook({ id: '1', name: 'A' }), false)).toBeNull();
  });
});
