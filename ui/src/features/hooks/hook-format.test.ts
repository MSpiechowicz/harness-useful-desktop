import { describe, expect, it } from 'vitest';

import type { Hook } from '$lib/ipc/bindings/Hook';
import type { HookStatus } from '$lib/ipc/bindings/HookStatus';
import type { HookView } from '$lib/ipc/bindings/HookView';

import { filterCounts, groupHooks, inStatus, inWhen, matchesQuery, pendingCount, waitingFor } from './hook-format';

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
  return { id: patch.id, hook: hook(patch), name: patch.name, status, sha256: 'a'.repeat(64), problem: null, approval: status === 'pending' ? 'never' : 'approved', approved: null };
}

const broken: HookView = { id: 'f'.repeat(32), hook: null, name: 'Broken', status: 'invalid', sha256: 'b'.repeat(64), problem: 'bad', approval: 'never', approved: null };

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
      status: { all: 5, active: 1, pending: 1, off: 1, paused: 1, invalid: 1 },
    });
  });

  it('count the hooks that wait for approval', () => {
    expect(pendingCount({ hooks: LIST, folderProblem: null })).toBe(1);
    expect(pendingCount(null)).toBe(0);
    expect(pendingCount({ hooks: [], folderProblem: null })).toBe(0);
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

describe('the groups', () => {
  it('put global hooks first, then the project ones, then the files to fix, with the total of each scope', () => {
    const groups = groupHooks(LIST, LIST, false);

    expect(groups.map((group) => [group.id, group.views.length, group.total])).toEqual([
      ['global', 2, 2],
      ['project', 2, 2],
      ['invalid', 1, 1],
    ]);
  });

  it('keep both scopes in the full list, even when one is empty', () => {
    const onlyGlobal = LIST.slice(0, 2);

    expect(groupHooks(onlyGlobal, onlyGlobal, false).map((group) => group.id)).toEqual(['global', 'project']);
  });

  it('leave out the groups a search or filter leaves empty, but still count the whole scope', () => {
    const shown = LIST.filter((item) => inWhen(item, 'toolEnd'));
    const groups = groupHooks(LIST, shown, true);

    expect(groups.map((group) => [group.id, group.views.length, group.total])).toEqual([['project', 1, 2]]);
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
