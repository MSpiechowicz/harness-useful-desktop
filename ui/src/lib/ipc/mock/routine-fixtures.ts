import type { Routine } from '../bindings/Routine';
import type { RoutineRun } from '../bindings/RoutineRun';
import type { RunStatus } from '../bindings/RunStatus';
import type { RunTrigger } from '../bindings/RunTrigger';
import { fixtures } from './fixtures';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** The project the sample project routines belong to. */
export const ROUTINES_PROJECT = fixtures.recentProjects[0].path;

/** The folder global routines run in and keep their sessions in. */
export const ROUTINES_WORKSPACE = '/home/demo/.local/share/harness-routines/workspace';

/** The repository the sample project routines of the team belong to: only this key travels, never a folder. */
export const ROUTINES_TEAM_REPO = 'git:github.com/acme/harness-demo';

export function ago(ms: number): string {
  return new Date(Date.now() - ms).toISOString().replace(/\.\d+Z$/, 'Z');
}

let counter = 0;

/** A 32 character id, like the real ones: stable per call order, so a preview looks the same on every load. */
export function routineId(): string {
  counter += 1;

  return `c${counter.toString(16).padStart(31, '0')}`;
}

export { fakeSha } from './fake-sha';

/** A routine of the desktop's own with the usual fields filled in. */
export function sample(patch: Partial<Routine> & Pick<Routine, 'name' | 'when' | 'action'>, age: number = 4 * DAY): Routine {
  return {
    id: routineId(),
    description: null,
    scope: 'global',
    project: null,
    enabled: true,
    createdAt: ago(age),
    createdBy: 'desktop',
    updatedAt: ago(age),
    updatedBy: 'desktop',
    ...patch,
  };
}

/** The sample routines of the default preview: one in every status the list can show. */
export function sampleRoutines(): { active: Routine[]; pending: Routine; changedApproved: Routine; changed: Routine } {
  const morning = sample(
    {
      name: 'Morning review',
      description: 'Goes through what happened overnight and sums it up before the day starts.',
      when: { type: 'weekdays', at: '09:00' },
      action: { type: 'agent', harness: 'omp', model: null, prompt: 'Review the open pull requests and the failing checks of this machine\'s projects.\nSum up what needs me first, in five lines at most.', approvalMode: 'write', trustOnce: false, maxMinutes: 20 },
    },
    9 * DAY,
  );
  const cleanup = sample({ name: 'Check the disk', when: { type: 'daily', at: '03:30' }, action: { type: 'command', command: 'du -sh ~/.cache ~/.local/share | sort -h', timeoutSecs: 60 } }, 14 * DAY);
  const memory = sample({ name: 'Look for hogs', when: { type: 'everyHours', every: 6 }, action: { type: 'script', name: 'disk-hogs', scope: 'global' } }, 6 * DAY);
  const tests = sample(
    { name: 'Run the tests', scope: 'project', project: ROUTINES_PROJECT, when: { type: 'everyMinutes', every: 30 }, action: { type: 'command', command: 'bun run test --reporter=dot', timeoutSecs: 600 } },
    3 * DAY,
  );
  const triage = sample(
    {
      name: 'Triage new issues',
      scope: 'project',
      project: ROUTINES_PROJECT,
      when: { type: 'cron', expression: '0 8-18/2 * * 1-5' },
      action: { type: 'agent', harness: 'omp', model: { provider: 'anthropic', id: 'claude-sonnet-4-5' }, prompt: 'Read the issues opened since the last run, label them and say which one to take next.', approvalMode: 'alwaysAsk', trustOnce: false, maxMinutes: null },
    },
    2 * DAY,
  );
  const startup = sample({ name: 'Check the build on start', scope: 'project', project: ROUTINES_PROJECT, enabled: false, when: { type: 'appStart' }, action: { type: 'command', command: 'cargo check --quiet', timeoutSecs: null } }, 5 * DAY);
  const changedApproved = sample(
    {
      name: 'Release notes',
      scope: 'project',
      project: ROUTINES_PROJECT,
      when: { type: 'weekdays', at: '16:45' },
      action: { type: 'agent', harness: 'pi', model: null, prompt: 'Draft the release notes from the commits since the last tag.', approvalMode: null, trustOnce: false, maxMinutes: 15 },
      createdBy: 'agent',
    },
    6 * DAY,
  );
  const changed: Routine = {
    ...changedApproved,
    when: { type: 'daily', at: '16:45' },
    action: { type: 'agent', harness: 'pi', model: null, prompt: 'Draft the release notes from the commits since the last tag.\nPublish them as a draft release when they are ready.', approvalMode: null, trustOnce: true, maxMinutes: 45 },
    updatedAt: ago(9 * MINUTE),
    updatedBy: 'agent',
  };
  const pending = sample(
    {
      name: 'Weekly changelog',
      description: 'Wrote itself into the routines folder, waiting for you.',
      when: { type: 'cron', expression: '0 8 * * 1' },
      action: { type: 'agent', harness: 'omp', model: null, prompt: 'Collect last week\'s merged work from every project and write the changelog.', approvalMode: 'yolo', trustOnce: false, maxMinutes: 60 },
      createdBy: 'agent',
      updatedBy: 'agent',
    },
    35 * MINUTE,
  );

  return { active: [morning, cleanup, memory, tests, triage, startup], pending, changedApproved, changed };
}

/** What a routine's run says last, by the kind of action: an agent's last reply, a command's output. */
const REPLIES = [
  'Two pull requests wait on review, and one check is red on main.',
  'Nothing needs you. The queue is empty and every check is green.',
  'Three new issues, all labelled. Take the flaky upload test next.',
  'The release notes are in the draft. Two entries still need a link.',
];

const OUTPUTS = ['ok  harness/ui 12.4s\n188 passed', '4.1G\t/home/demo/.cache\n9.8G\t/home/demo/.local/share', 'Checking harness_business v0.25.0\nFinished dev profile in 6.2s'];

/** The routine a run is of, as far as a run record needs to know. */
export interface RunSource {
  id: string;
  name: string;
  kind: 'agent' | 'command' | 'script';
  project: string | null;
}

const STATUS_CYCLE: readonly RunStatus[] = ['ok', 'ok', 'ok', 'failed', 'ok', 'ok', 'stopped', 'ok', 'needsAttention', 'ok'];
const TRIGGER_CYCLE: readonly RunTrigger[] = ['schedule', 'schedule', 'manual', 'schedule', 'catchUp', 'schedule', 'appStart'];

/** The newest-first history of the sample routines: `count` finished runs, spread back over the last weeks, in a repeating pattern of outcomes. */
export function sampleRuns(sources: readonly RunSource[], count: number): RoutineRun[] {
  return Array.from({ length: count }, (_, index) => {
    const source = sources[index % sources.length];
    const status = STATUS_CYCLE[index % STATUS_CYCLE.length];
    const startedMs = (index + 1) * 53 * MINUTE + (index % 7) * 4 * MINUTE;
    const duration = (20 + ((index * 37) % 400)) * 1000;
    const agent = source.kind === 'agent';

    return {
      id: `d${(index + 1).toString(16).padStart(31, '0')}`,
      routineId: source.id,
      needsYou: false,
      routineName: source.name,
      origin: 'local',
      trigger: TRIGGER_CYCLE[index % TRIGGER_CYCLE.length],
      startedAt: ago(startedMs),
      endedAt: ago(startedMs - duration),
      durationMs: duration,
      status,
      message: runMessage(status),
      excerpt: status === 'ok' || status === 'failed' ? (agent ? REPLIES[index % REPLIES.length] : OUTPUTS[index % OUTPUTS.length]) : null,
      errorExcerpt: status === 'failed' && !agent ? 'error: the command did not finish cleanly\nexit status 1' : null,
      exitCode: agent ? null : status === 'ok' ? 0 : status === 'failed' ? 1 : null,
      session: agent ? { harness: 'omp', id: `s-${index + 1}` } : null,
      project: source.project,
    };
  });
}

function runMessage(status: RunStatus): string | null {
  switch (status) {
    case 'failed':
      return 'The run ended with an error.';
    case 'stopped':
      return 'Stopped by you.';
    case 'needsAttention':
      return 'Trust the script in the Scripts view, then run it again.';
    default:
      return null;
  }
}

/** A run that goes on now. */
export function liveRun(source: RunSource, needsYou: boolean, startedAgo: number): RoutineRun {
  const agent = source.kind === 'agent';

  return {
    id: `e${source.id.slice(1)}`,
    routineId: source.id,
    needsYou,
    routineName: source.name,
    origin: 'local',
    trigger: 'schedule',
    startedAt: ago(startedAgo),
    endedAt: null,
    durationMs: null,
    status: 'running',
    message: null,
    excerpt: null,
    errorExcerpt: null,
    exitCode: null,
    session: agent ? { harness: 'omp', id: `s-live-${source.id.slice(-4)}` } : null,
    project: source.project,
  };
}
