import { beforeAll, describe, expect, it } from 'vitest';

import type { Routine } from '$lib/ipc/bindings/Routine';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import { fixtures } from '$lib/ipc/mock/fixtures';
import { withOpenCodeChat } from '$lib/ipc/mock/opencode-chat';
import { harness } from '$lib/stores/harness.svelte';

import { changesOf } from './routine-diff';
import { routineFacts } from './routine-facts';

// The harness list says which harness asks for approval how.
beforeAll(() => {
  harness.summaries = fixtures.harnesses.map(withOpenCodeChat);
});

const BASE: Routine = {
  id: 'a'.repeat(32),
  name: 'Morning review',
  description: null,
  scope: 'project',
  project: '/work/app',
  when: { type: 'weekdays', at: '09:00' },
  action: { type: 'agent', harness: 'omp', model: null, prompt: 'Review the open pull requests.', approvalMode: null, trustOnce: false, maxMinutes: null },
  enabled: true,
  createdAt: '2026-10-17T10:00:00Z',
  createdBy: 'agent',
  updatedAt: '2026-10-17T10:00:00Z',
  updatedBy: 'desktop',
};

describe('changesOf', () => {
  it('lists only what differs from the approved version, worded for a reader', () => {
    const waiting: Routine = { ...BASE, when: { type: 'daily', at: '10:30' }, action: { ...(BASE.action as Extract<Routine['action'], { type: 'agent' }>), prompt: 'Review the open pull requests.\nThen publish a draft.', approvalMode: 'yolo', maxMinutes: 45 } };

    expect(changesOf(waiting, BASE)).toEqual([
      { field: 'when', from: 'Weekdays at 09:00', to: 'Every day at 10:30' },
      { field: 'approval', from: 'Harness setting', to: 'Yolo' },
      { field: 'maxMinutes', from: '30 min', to: '45 min' },
      { field: 'text', from: 'Review the open pull requests.', to: 'Review the open pull requests.\nThen publish a draft.' },
    ]);
  });

  it('lists every part an agent set when nothing was approved yet, and none it left empty', () => {
    const fields = changesOf(BASE, null).map((change) => change.field);

    expect(fields).toEqual(['name', 'when', 'does', 'harness', 'model', 'approval', 'maxMinutes', 'text', 'scope', 'project', 'enabled']);
    expect(changesOf(BASE, null).find((change) => change.field === 'description')).toBeUndefined();
    expect(changesOf(BASE, null).every((change) => change.from === null)).toBe(true);
  });

  it('compares the parts of a command and a script by their own', () => {
    const command: Routine = { ...BASE, action: { type: 'command', command: 'bun test', timeoutSecs: null } };
    const slower: Routine = { ...BASE, action: { type: 'command', command: 'bun test', timeoutSecs: 600 } };
    const script: Routine = { ...BASE, action: { type: 'script', name: 'disk-hogs', scope: 'repo' } };

    expect(changesOf(slower, command)).toEqual([{ field: 'timeout', from: '300 s', to: '600 s' }]);
    expect(changesOf(script, command).map((change) => change.field)).toEqual(['does', 'text', 'timeout', 'scriptScope']);
  });

  it('says nothing when nothing changed', () => {
    expect(changesOf(BASE, BASE)).toEqual([]);
  });
});

describe('routineFacts', () => {
  function view(patch: Partial<RoutineView> = {}): RoutineView {
    return {
      id: BASE.id,
      routine: BASE,
      name: BASE.name,
      status: 'active',
      sha256: 'b'.repeat(64),
      problem: null,
      approval: 'approved',
      approved: null,
      origin: 'local',
      runsUnattended: false,
      canRunNow: true,
      queued: false,
      nextRun: null,
      teamAuthor: null,
      teamUpdatedAt: null,
      teamRepo: null,
      teamChanged: false,
      teamOn: false,
      teamFolder: null,
      shareBlock: null,
      root: null,
      repoKey: null,
      ...patch,
    };
  }

  function labels(rows: ReturnType<typeof routineFacts>): string[] {
    return rows.map((row) => row.label);
  }

  it('gives the same facts in the same order for every routine of a kind, with the ones that do not apply left out', () => {
    expect(labels(routineFacts(BASE, 'approved', view()))).toEqual(['When', 'Does', 'Harness', 'Model', 'Approval mode', 'Stop after', 'Applies to', 'Project', 'Source', 'Changed']);
    expect(labels(routineFacts({ ...BASE, scope: 'global', project: null, action: { type: 'command', command: 'ls', timeoutSecs: null } }, 'approved', view()))).toEqual(['When', 'Does', 'Timeout', 'Applies to', 'Source', 'Changed']);
  });

  it('names the next run, and the whole folder of a project routine', () => {
    const rows = routineFacts(BASE, 'approved', view({ nextRun: '2026-10-19T09:00:00Z' }), Date.parse('2026-10-18T12:00:00Z'));

    expect(rows.find((row) => row.label === 'Next run')).toMatchObject({ value: 'in 21 hours' });
    expect(rows.find((row) => row.label === 'Project')).toMatchObject({ value: '/work/app', mono: true });
  });

  it('marks what runs tools without asking', () => {
    const yolo = { ...BASE, action: { ...(BASE.action as Extract<Routine['action'], { type: 'agent' }>), approvalMode: 'yolo' as const } };
    const trusting = { ...BASE, action: { type: 'agent' as const, harness: 'pi' as const, model: null, prompt: 'Go.', approvalMode: null, trustOnce: true, maxMinutes: null } };

    expect(routineFacts(yolo, 'approved', view()).find((row) => row.label === 'Approval mode')).toMatchObject({ value: 'Yolo', tone: 'warning' });
    expect(routineFacts(trusting, 'approved', view()).find((row) => row.label === 'Project trust')).toMatchObject({ tone: 'warning' });
  });

  it('gives an OpenCode agent an approval mode and no project trust, as its capabilities say', () => {
    const opencode = { ...BASE, action: { type: 'agent' as const, harness: 'opencode' as const, model: null, prompt: 'Go.', approvalMode: 'write' as const, trustOnce: false, maxMinutes: null } };
    const rows = routineFacts(opencode, 'approved', view());

    expect(rows.find((row) => row.label === 'Approval mode')).toMatchObject({ value: 'Write' });
    expect(rows.find((row) => row.label === 'Project trust')).toBeUndefined();
    expect(changesOf(opencode, { ...opencode, action: { ...opencode.action, approvalMode: 'yolo' } })).toEqual([{ field: 'approval', from: 'Yolo', to: 'Write' }]);
  });

  it('credits a routine to an agent while its version waits, and says for a remote routine who shared it', () => {
    const waiting = { ...BASE, createdBy: 'desktop' as const, updatedBy: 'agent' as const };

    expect(routineFacts(waiting, 'changed', view()).find((row) => row.label === 'Source')).toMatchObject({ value: 'An agent' });
    expect(routineFacts(waiting, 'approved', view()).find((row) => row.label === 'Source')).toMatchObject({ value: 'You' });

    const team = routineFacts(BASE, 'never', view({ origin: 'team', teamAuthor: 'Anna', teamUpdatedAt: '2026-10-17T10:00:00Z', teamRepo: 'git:github.com/acme/app', teamFolder: '/work/app' }));

    expect(labels(team)).toContain('Shared by');
    expect(team.find((row) => row.label === 'Repository')).toMatchObject({ value: 'github.com/acme/app' });
    expect(team.find((row) => row.label === 'Runs in')).toMatchObject({ value: '/work/app' });
    expect(team.find((row) => row.label === 'Source')).toMatchObject({ value: 'Remote' });
  });
});
