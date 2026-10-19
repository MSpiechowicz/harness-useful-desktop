import Bot from '@lucide/svelte/icons/bot';
import CalendarDays from '@lucide/svelte/icons/calendar-days';
import Clock from '@lucide/svelte/icons/clock';
import FileCode from '@lucide/svelte/icons/file-code';
import Folder from '@lucide/svelte/icons/folder';
import Globe from '@lucide/svelte/icons/globe';
import Hourglass from '@lucide/svelte/icons/hourglass';
import Power from '@lucide/svelte/icons/power';
import Repeat from '@lucide/svelte/icons/repeat';
import SquareTerminal from '@lucide/svelte/icons/square-terminal';
import Timer from '@lucide/svelte/icons/timer';

import { type DocGroup, groupDocs, MAX_PER_SCOPE } from '$lib/components/docs/doc-groups';
import type { IconComponent } from '$lib/components/icon';
import { t, tn } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HookScope } from '$lib/ipc/bindings/HookScope';
import type { RoutineAction } from '$lib/ipc/bindings/RoutineAction';
import type { RoutineCounts } from '$lib/ipc/bindings/RoutineCounts';
import type { RoutineListing } from '$lib/ipc/bindings/RoutineListing';
import type { RoutineRun } from '$lib/ipc/bindings/RoutineRun';
import type { RoutineStatus } from '$lib/ipc/bindings/RoutineStatus';
import type { RoutineView } from '$lib/ipc/bindings/RoutineView';
import type { RoutineWhen } from '$lib/ipc/bindings/RoutineWhen';
import type { RoutineWhenKind } from '$lib/ipc/bindings/RoutineWhenKind';
import type { RunStatus } from '$lib/ipc/bindings/RunStatus';
import type { RunTrigger } from '$lib/ipc/bindings/RunTrigger';
import type { RoutineSource, RoutineStatusFilter, RoutineWhenFilter } from '$lib/stores/view-state.svelte';
import { perLocale } from '$lib/utils/format';

import { spelledOut } from '../memory/script-format';

export { MAX_PER_SCOPE };

export type ActionKind = RoutineAction['type'];

/** The ways to schedule a routine, in the order they are offered. */
export const WHEN_KINDS: readonly RoutineWhenKind[] = ['daily', 'weekdays', 'everyMinutes', 'everyHours', 'cron', 'appStart'];

export const WHEN_LABELS: Record<RoutineWhenKind, MessageKey> = {
  daily: 'routines.when.daily',
  weekdays: 'routines.when.weekdays',
  everyMinutes: 'routines.when.everyMinutes',
  everyHours: 'routines.when.everyHours',
  cron: 'routines.when.cron',
  appStart: 'routines.when.appStart',
};

/** One line under a way to schedule: what it means. */
export const WHEN_HINTS: Record<RoutineWhenKind, MessageKey> = {
  daily: 'routines.when.daily.hint',
  weekdays: 'routines.when.weekdays.hint',
  everyMinutes: 'routines.when.everyMinutes.hint',
  everyHours: 'routines.when.everyHours.hint',
  cron: 'routines.when.cron.hint',
  appStart: 'routines.when.appStart.hint',
};

export const WHEN_ICONS: Record<RoutineWhenKind, IconComponent> = {
  daily: Clock,
  weekdays: CalendarDays,
  everyMinutes: Timer,
  everyHours: Hourglass,
  cron: Repeat,
  appStart: Power,
};

/** The kinds of action in the order the editor offers them. */
export const ACTION_KINDS: readonly ActionKind[] = ['agent', 'command', 'script'];

export const ACTION_LABELS: Record<ActionKind, MessageKey> = {
  agent: 'routines.action.agent',
  command: 'routines.action.command',
  script: 'routines.action.script',
};

export const ACTION_ICONS: Record<ActionKind, IconComponent> = { agent: Bot, command: SquareTerminal, script: FileCode };

export const SCOPE_LABELS: Record<HookScope, MessageKey> = { global: 'routines.scope.global', project: 'routines.scope.project' };

export const SCOPE_ICONS: Record<HookScope, IconComponent> = { global: Globe, project: Folder };

/** What a harness is called on screen. These are product names and stay the same in every language. */
export const HARNESS_NAMES: Record<HarnessId, string> = { omp: 'OMP', pi: 'PI', opencode: 'OpenCode' };

/** The harnesses a routine can start a chat with (OpenCode has no chat yet). */
export const ROUTINE_HARNESSES: readonly HarnessId[] = ['omp', 'pi'];

export const STATUS_LABELS: Record<RoutineStatus, MessageKey> = {
  active: 'routines.status.active',
  running: 'routines.status.running',
  off: 'routines.status.off',
  pending: 'routines.status.pending',
  paused: 'routines.status.paused',
  invalid: 'routines.status.invalid',
  duplicate: 'routines.status.duplicate',
};

export const STATUS_TONES = { active: 'success', running: 'accent', off: 'neutral', pending: 'warning', paused: 'info', invalid: 'danger', duplicate: 'neutral' } as const satisfies Record<RoutineStatus, string>;

/** The statuses in the order the Status filter lists them. */
export const STATUSES: readonly RoutineStatus[] = ['active', 'running', 'pending', 'off', 'paused', 'invalid', 'duplicate'];

export const RUN_STATUS_LABELS: Record<RunStatus, MessageKey> = {
  running: 'routines.run.status.running',
  ok: 'routines.run.status.ok',
  failed: 'routines.run.status.failed',
  needsAttention: 'routines.run.status.needsAttention',
  stopped: 'routines.run.status.stopped',
};

export const RUN_STATUS_TONES = { running: 'accent', ok: 'success', failed: 'danger', needsAttention: 'warning', stopped: 'neutral' } as const satisfies Record<RunStatus, string>;

/** The run statuses in the order the run history's Status filter lists them. */
export const RUN_STATUSES: readonly RunStatus[] = ['running', 'ok', 'failed', 'needsAttention', 'stopped'];

export const TRIGGER_LABELS: Record<RunTrigger, MessageKey> = {
  schedule: 'routines.trigger.schedule',
  catchUp: 'routines.trigger.catchUp',
  appStart: 'routines.trigger.appStart',
  manual: 'routines.trigger.manual',
};

/** The key a routine is listed, opened and worked on by: a team routine can share its id with a local one, so the team's carries a prefix. */
export function viewKey(view: RoutineView): string {
  return view.origin === 'team' ? `team:${view.id}` : view.id;
}

/** Whether a run of the routine waits for a free place (the places for runs are all taken): it starts by itself, and asking again would change nothing. */
export function isQueued(view: RoutineView): boolean {
  return view.queued;
}

/** The name as the file has it: an invalid file may have none, then its id stands in. */
function rawTitle(view: RoutineView): string {
  return view.routine?.name ?? view.name ?? view.id;
}

/** The name a routine is listed under. A file an agent wrote may hide characters in it, so each is spelled out. */
export function routineTitle(view: RoutineView): string {
  return spelledOut(rawTitle(view));
}

/** The words for when a routine runs: "Weekdays at 09:00", "Every 15 minutes". A cron expression is shown as it is written. */
export function whenText(when: RoutineWhen): string {
  switch (when.type) {
    case 'daily':
      return t('routines.when.text.daily', { at: when.at });
    case 'weekdays':
      return t('routines.when.text.weekdays', { at: when.at });
    case 'everyMinutes':
      return tn('routines.when.text.everyMinutes', when.every);
    case 'everyHours':
      return tn('routines.when.text.everyHours', when.every);
    case 'cron':
      return when.expression;
    case 'appStart':
      return t('routines.when.text.appStart');
  }
}

/** The text a routine sends or the command it runs; for a script, its name. */
export function actionText(action: RoutineAction): string {
  switch (action.type) {
    case 'agent':
      return action.prompt;
    case 'command':
      return action.command;
    case 'script':
      return action.name;
  }
}

/** What a routine does in a few words: "OMP agent", "Command", "Script deploy-check". */
export function doesText(action: RoutineAction): string {
  switch (action.type) {
    case 'agent':
      return t('routines.does.agent', { harness: HARNESS_NAMES[action.harness] });
    case 'command':
      return t('routines.does.command');
    case 'script':
      return t('routines.does.script', { name: action.name });
  }
}

/** The routines the Routines view and its rail badge count as waiting for the user's approval. */
export function pendingCount(listing: RoutineListing | null): number {
  return listing?.counts.status.pending ?? 0;
}

export function inWhen(view: RoutineView, when: RoutineWhenFilter): boolean {
  return when === 'all' || view.routine?.when.type === when;
}

export function inStatus(view: RoutineView, status: RoutineStatusFilter): boolean {
  return status === 'all' || view.status === status;
}

export function inSource(view: RoutineView, source: RoutineSource): boolean {
  return source === 'all' || (source === 'team') === (view.origin === 'team');
}

/** Whether a routine answers the search: its name, description, prompt, command or script, and cron expression. */
export function matchesQuery(view: RoutineView, query: string): boolean {
  const needle = query.trim().toLowerCase();

  if (needle === '') {
    return true;
  }

  const routine = view.routine;
  const haystack = [rawTitle(view), routine?.description, routine ? actionText(routine.action) : view.problem, routine?.when.type === 'cron' ? routine.when.expression : null];

  return haystack.some((part) => part?.toLowerCase().includes(needle));
}

/** How many routines each choice of the three filters holds (of all the routines, not of the search or the other filters). */
export interface FilterCounts {
  when: Record<RoutineWhenFilter, number>;
  status: Record<RoutineStatusFilter, number>;
  source: Record<RoutineSource, number>;
}

/** The counts of a listing, which the backend takes over every routine it lists whatever the filters, as the menus of the filters show them. */
export function filterCounts(counts: RoutineCounts): FilterCounts {
  const when = { all: counts.total, ...Object.fromEntries(WHEN_KINDS.map((kind) => [kind, counts.when[kind] ?? 0])) } as FilterCounts['when'];
  const status = { all: counts.total, ...Object.fromEntries(STATUSES.map((value) => [value, counts.status[value] ?? 0])) } as FilterCounts['status'];

  return { when, status, source: { all: counts.total, mine: counts.origin.local ?? 0, team: counts.origin.team ?? 0 } };
}

/** One block of the list: the global routines, the project's, the files that are not routines (invalid), or the team's. */
export type RoutineGroup = DocGroup<RoutineView>;

/** The routines that answer the search and the filters, in their groups (see `groupDocs`). */
export function groupRoutines(all: RoutineView[], shown: RoutineView[], narrowed: boolean, teamOn = false): RoutineGroup[] {
  return groupDocs(all, shown, narrowed, teamOn, (view) => (view.origin === 'team' ? 'team' : (view.routine?.scope ?? 'invalid')));
}

/** The last part of a folder path: the name a project is shown by. */
export function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

const FUTURE = perLocale((l) => new Intl.RelativeTimeFormat(l, { numeric: 'auto' }));
const SLOT = perLocale((l) => new Intl.DateTimeFormat(l, { weekday: 'short', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }));
const UNIT = perLocale((l) => (unit: 'second' | 'minute' | 'hour', value: number) => new Intl.NumberFormat(l, { style: 'unit', unit, unitDisplay: 'narrow' }).format(value));

/** A moment still to come as people read it: "in 3 hours", "tomorrow"; empty for a value that is not a date. */
export function untilTime(iso: string, now: number = Date.now()): string {
  const ahead = Date.parse(iso) - now;

  if (Number.isNaN(ahead)) {
    return '';
  }

  if (ahead < MINUTE) {
    return t('routines.next.soon');
  }

  if (ahead < HOUR) {
    return FUTURE().format(Math.floor(ahead / MINUTE), 'minute');
  }

  if (ahead < DAY) {
    return FUTURE().format(Math.floor(ahead / HOUR), 'hour');
  }

  return FUTURE().format(Math.floor(ahead / DAY), 'day');
}

/** How long a run took in as few characters as the language allows: "42s", "3m 12s", "1h 5m". */
export function durationText(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000));
  const unit = UNIT();

  if (seconds < 60) {
    return unit('second', seconds);
  }

  const minutes = Math.floor(seconds / 60);

  if (minutes < 60) {
    return seconds % 60 === 0 ? unit('minute', minutes) : `${unit('minute', minutes)} ${unit('second', seconds % 60)}`;
  }

  const hours = Math.floor(minutes / 60);

  return minutes % 60 === 0 ? unit('hour', hours) : `${unit('hour', hours)} ${unit('minute', minutes % 60)}`;
}

/** The first line of a run's result: what the agent said last, what the command printed, or why it did not go well. */
export function resultLine(run: RoutineRun): string {
  const text = [run.excerpt, run.message, run.errorExcerpt].find((part) => part !== null && part.trim() !== '');
  const line = text?.trim().split(/\r?\n/)[0] ?? '';

  return spelledOut(line);
}

/** The routines a run waits on: those with a run that asks the user something, by routine id. */
export function waitingRoutines(runs: Iterable<RoutineRun>): Map<string, string> {
  const waiting = new Map<string, string>();

  for (const run of runs) {
    if (run.status === 'running' && run.needsYou) {
      waiting.set(run.routineId, run.id);
    }
  }

  return waiting;
}

/** One scheduled moment as the preview of a schedule lists it: "Mon, Oct 19, 09:00 AM"; empty for a value that is not a date. */
export function slotText(iso: string): string {
  const date = new Date(iso);

  return Number.isNaN(date.getTime()) ? '' : SLOT().format(date);
}
