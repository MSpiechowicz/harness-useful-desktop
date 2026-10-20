import { describe, expect, it } from 'vitest';

import type { RoutineCounts } from '$lib/ipc/bindings/RoutineCounts';
import type { Routine } from '$lib/ipc/bindings/Routine';
import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import type { RoutineWhen } from '$lib/ipc/bindings/RoutineWhen';

import { actionText, doesText, durationText, filterCounts, groupRoutines, inSource, inStatus, inWhen, matchesQuery, pendingCount, resultLine, routineTitle, slotText, untilTime, viewKey, waitingRoutines, whenText } from './routine-format';

const ROUTINE: Routine = {
  id: 'a'.repeat(32),
  name: 'Morning review',
  description: 'Goes through the night.',
  scope: 'global',
  project: null,
  when: { type: 'weekdays', at: '09:00' },
  action: { type: 'agent', harness: 'omp', model: null, prompt: 'Review the open pull requests.', approvalMode: null, trustOnce: false, maxMinutes: null },
  enabled: true,
  createdAt: '2026-10-17T10:00:00Z',
  createdBy: 'desktop',
  updatedAt: '2026-10-17T10:00:00Z',
  updatedBy: 'desktop',
};

function view(patch: Partial<Omit<RoutineView, 'routine'>> & { routine?: Partial<Routine> | null } = {}): RoutineView {
  const { routine, ...rest } = patch;

  return {
    id: ROUTINE.id,
    routine: routine === null ? null : { ...ROUTINE, ...routine },
    name: ROUTINE.name,
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
    ...rest,
  };
}

describe('whenText', () => {
  it.each<[RoutineWhen, string]>([
    [{ type: 'daily', at: '07:30' }, 'Every day at 07:30'],
    [{ type: 'weekdays', at: '09:00' }, 'Weekdays at 09:00'],
    [{ type: 'everyMinutes', every: 1 }, 'Every minute'],
    [{ type: 'everyMinutes', every: 15 }, 'Every 15 minutes'],
    [{ type: 'everyHours', every: 1 }, 'Every hour'],
    [{ type: 'everyHours', every: 6 }, 'Every 6 hours'],
    [{ type: 'cron', expression: '*/5 9-17 * * mon-fri' }, '*/5 9-17 * * mon-fri'],
    [{ type: 'appStart' }, 'When the app starts'],
  ])('writes %j as "%s"', (when, text) => {
    expect(whenText(when)).toBe(text);
  });
});

describe('what a routine does', () => {
  it('says it in a few words, and gives the text it sends or the command it runs', () => {
    expect(doesText(ROUTINE.action)).toBe('OMP agent');
    expect(doesText({ type: 'command', command: 'ls', timeoutSecs: null })).toBe('Command');
    expect(doesText({ type: 'script', name: 'disk-hogs', scope: 'global' })).toBe('Script disk-hogs');
    expect(actionText(ROUTINE.action)).toBe('Review the open pull requests.');
    expect(actionText({ type: 'command', command: 'ls -la', timeoutSecs: null })).toBe('ls -la');
    expect(actionText({ type: 'script', name: 'disk-hogs', scope: 'repo' })).toBe('disk-hogs');
  });
});

describe('titles and keys', () => {
  it('spells out the hidden characters an agent put in a name, and falls back to the id for a file with none', () => {
    expect(routineTitle(view({ routine: { name: 'Back‮up' } }))).toBe('Back⟨U+202E⟩up');
    expect(routineTitle(view({ routine: null, name: null }))).toBe(ROUTINE.id);
  });

  it('keys a team routine apart from a local one with the same id', () => {
    expect(viewKey(view())).toBe(ROUTINE.id);
    expect(viewKey(view({ origin: 'team' }))).toBe(`team:${ROUTINE.id}`);
  });
});

describe('searching and filtering', () => {
  it('matches the name, the description, the prompt or command, and a cron expression', () => {
    expect(matchesQuery(view(), 'MORNING')).toBe(true);
    expect(matchesQuery(view(), 'the night')).toBe(true);
    expect(matchesQuery(view(), 'pull requests')).toBe(true);
    expect(matchesQuery(view({ routine: { when: { type: 'cron', expression: '0 8 * * 1' } } }), '8 * *')).toBe(true);
    expect(matchesQuery(view(), 'nothing like it')).toBe(false);
    expect(matchesQuery(view(), '   ')).toBe(true);
  });

  it('keeps a routine by its kind of schedule, its status and its source', () => {
    expect(inWhen(view(), 'weekdays')).toBe(true);
    expect(inWhen(view(), 'cron')).toBe(false);
    expect(inWhen(view({ routine: null }), 'all')).toBe(true);
    expect(inStatus(view({ status: 'pending' }), 'pending')).toBe(true);
    expect(inStatus(view({ status: 'pending' }), 'off')).toBe(false);
    expect(inSource(view({ origin: 'team' }), 'team')).toBe(true);
    expect(inSource(view({ origin: 'team' }), 'mine')).toBe(false);
    expect(inSource(view(), 'mine')).toBe(true);
  });

  it('counts the choices of the filters from the counts of the backend, with zero for the ones it did not name', () => {
    const counts: RoutineCounts = { total: 5, status: { active: 3, pending: 2 }, when: { daily: 1, cron: 4 }, origin: { local: 4, team: 1 } };

    expect(filterCounts(counts)).toEqual({
      when: { all: 5, daily: 1, weekdays: 0, everyMinutes: 0, everyHours: 0, cron: 4, appStart: 0 },
      status: { all: 5, active: 3, running: 0, pending: 2, off: 0, paused: 0, invalid: 0, duplicate: 0 },
      source: { all: 5, mine: 4, team: 1 },
    });
  });

  it('counts the routines that wait for approval', () => {
    const counts: RoutineCounts = { total: 3, status: { pending: 2 }, when: {}, origin: {} };

    expect(pendingCount({ routines: [], folderProblem: null, team: null, counts })).toBe(2);
    expect(pendingCount(null)).toBe(0);
  });
});

describe('groupRoutines', () => {
  const GLOBAL = view({ id: '1'.repeat(32) });
  const PROJECT = view({ id: '2'.repeat(32), routine: { scope: 'project', project: '/work/app' } });
  const BROKEN = view({ id: '3'.repeat(32), routine: null, status: 'invalid' });
  const TEAM = view({ id: '4'.repeat(32), origin: 'team' });

  it('puts the routines in the global, project, invalid and team groups, with how many each holds in all', () => {
    const groups = groupRoutines([GLOBAL, PROJECT, BROKEN, TEAM], [GLOBAL, PROJECT, BROKEN, TEAM], false, true);

    expect(groups.map((group) => [group.id, group.views.length, group.total])).toEqual([['global', 1, 1], ['project', 1, 1], ['invalid', 1, 1], ['team', 1, 1]]);
  });

  it('shows both scopes while empty, hides the invalid and the team group, and leaves out what a search leaves empty', () => {
    expect(groupRoutines([GLOBAL], [GLOBAL], false).map((group) => group.id)).toEqual(['global', 'project']);
    expect(groupRoutines([GLOBAL], [GLOBAL], false, true).map((group) => group.id)).toEqual(['global', 'project', 'team']);
    expect(groupRoutines([GLOBAL, PROJECT], [PROJECT], true).map((group) => [group.id, group.total])).toEqual([['project', 1]]);
  });
});

describe('moments and durations', () => {
  const NOW = Date.parse('2026-10-18T12:00:00Z');

  it('says when a run is due, from "soon" to days', () => {
    expect(untilTime('2026-10-18T12:00:30Z', NOW)).toBe('in under a minute');
    expect(untilTime('2026-10-18T12:15:00Z', NOW)).toBe('in 15 minutes');
    expect(untilTime('2026-10-18T15:30:00Z', NOW)).toBe('in 3 hours');
    expect(untilTime('2026-10-20T12:00:00Z', NOW)).toBe('in 2 days');
    expect(untilTime('not a date', NOW)).toBe('');
  });

  it('writes how long a run took in as few characters as the language allows', () => {
    expect(durationText(800)).toBe('1s');
    expect(durationText(42_000)).toBe('42s');
    expect(durationText(192_000)).toBe('3m 12s');
    expect(durationText(300_000)).toBe('5m');
    expect(durationText(3_900_000)).toBe('1h 5m');
    expect(durationText(7_200_000)).toBe('2h');
  });

  it('writes a moment of a schedule as a short date and time, and nothing for a value that is not one', () => {
    expect(slotText('2026-10-19T09:00:00Z')).toMatch(/Oct/);
    expect(slotText('soon')).toBe('');
  });
});

describe('runs', () => {
  const RUN: RoutineRun = {
    routineId: ROUTINE.id,
    needsYou: false,
    id: 'c'.repeat(32),
    routineName: 'Morning review',
    origin: 'local',
    trigger: 'schedule',
    startedAt: '2026-10-18T09:00:00Z',
    endedAt: '2026-10-18T09:03:12Z',
    durationMs: 192_000,
    status: 'ok',
    message: null,
    excerpt: 'Two pull requests wait.\nOne check is red.',
    errorExcerpt: null,
    exitCode: null,
    session: null,
    project: null,
  };

  it('shows the first line of what the run said, or why it did not go well', () => {
    expect(resultLine(RUN)).toBe('Two pull requests wait.');
    expect(resultLine({ ...RUN, excerpt: null, message: 'Stopped by you.' })).toBe('Stopped by you.');
    expect(resultLine({ ...RUN, excerpt: '  ', message: null, errorExcerpt: 'error: boom' })).toBe('error: boom');
    expect(resultLine({ ...RUN, excerpt: null })).toBe('');
  });

  it('finds the routines with a run that waits for an answer, and nothing else', () => {
    const waiting = { ...RUN, id: 'd'.repeat(32), routineId: 'e'.repeat(32), status: 'running' as const, needsYou: true };
    const going = { ...RUN, id: 'f'.repeat(32), routineId: '1'.repeat(32), status: 'running' as const };

    expect(waitingRoutines([RUN, going, waiting])).toEqual(new Map([['e'.repeat(32), 'd'.repeat(32)]]));
  });
});
