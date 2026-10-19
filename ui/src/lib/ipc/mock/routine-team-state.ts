import type { Routine } from '../bindings/Routine';
import type { RoutineStatus } from '../bindings/RoutineStatus';
import type { RoutineView } from '../bindings/RoutineView';
import type { ShareBlock } from '../bindings/ShareBlock';
import type { TeamRoutines } from '../bindings/TeamRoutines';
import { IpcFailure } from '../transport';
import { ago, fakeSha, ROUTINES_PROJECT, ROUTINES_TEAM_REPO, sample } from './routine-fixtures';
import type { TeamMode } from './team-preview';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** The fields every local view has for the team: none of them apply. */
export const LOCAL_ORIGIN = { origin: 'local', teamAuthor: null, teamUpdatedAt: null, teamRepo: null, teamChanged: false, teamOn: false, teamFolder: null } as const;

/** One routine on the team database. */
interface Row {
  routine: Routine;
  author: string;
  updatedAt: string;
  /** The version this computer approved; null when none was. */
  approved: Routine | null;
  /** Turned on for this computer. */
  on: boolean;
  /** The folder a project routine runs in here. */
  folder: string | null;
}

/** A routine as it is stored on the team: a project routine carries its repository key, not a folder. */
export function teamBody(routine: Routine): Routine {
  return { ...routine, project: routine.scope === 'project' ? ROUTINES_TEAM_REPO : null };
}

/** The team routines of the previews, in the version this computer approved where it did. */
function sampleRows(): Row[] {
  const standup = sample({ name: 'Standup summary', when: { type: 'weekdays', at: '09:30' }, action: { type: 'agent', harness: 'omp', model: null, prompt: 'Sum up yesterday\'s merged work for the standup, in five lines.', approvalMode: 'write', trustOnce: false, maxMinutes: 10 } }, 20 * DAY);
  const audit = sample(
    { name: 'Dependency audit', scope: 'project', project: ROUTINES_TEAM_REPO, when: { type: 'daily', at: '07:15' }, action: { type: 'command', command: 'bun audit', timeoutSecs: 120 } },
    12 * DAY,
  );
  const sweep = sample(
    { name: 'Sweep stale branches', when: { type: 'cron', expression: '0 18 * * 5' }, action: { type: 'agent', harness: 'omp', model: null, prompt: 'List the merged branches older than a month and delete them.', approvalMode: 'yolo', trustOnce: false, maxMinutes: 30 } },
    3 * HOUR,
  );

  return [
    { routine: standup, author: 'Anna Kowalska', updatedAt: ago(20 * DAY), approved: standup, on: true, folder: null },
    { routine: audit, author: 'Marek Nowak', updatedAt: ago(2 * DAY), approved: null, on: false, folder: null },
    { routine: sweep, author: 'Anna Kowalska', updatedAt: ago(3 * HOUR), approved: null, on: true, folder: null },
  ];
}

/** The team routines of the mock backend, kept in memory: the cache the real desktop keeps of the team database, and what it approved. */
export class MockRoutineTeam {
  private rows = new Map<string, Row>();
  /** The local version each routine was shared from, by id. */
  private sharedFrom = new Map<string, string>();
  private refreshedAt = ago(MINUTE);

  constructor(readonly mode: TeamMode | null) {
    this.rows = new Map(mode ? sampleRows().map((row) => [row.routine.id, row]) : []);
  }

  get on(): boolean {
    return this.mode !== null;
  }

  /** What `routines_list` says of the team: null while routines are not shared. */
  state(): TeamRoutines | null {
    return this.mode ? { trust: this.mode.trust, problem: this.mode.problem, refreshedAt: this.refreshedAt } : null;
  }

  refresh(): void {
    this.refreshedAt = ago(0);
  }

  /** Why a local routine cannot be shared, or null when it can. */
  shareBlock(routine: Routine, pending: boolean): ShareBlock | null {
    if (pending) {
      return 'pending';
    }

    if (this.rows.has(routine.id)) {
      return 'onTeam';
    }

    return routine.scope === 'project' && !this.mode?.shareable ? 'noOrigin' : null;
  }

  /** Puts the local routine on the team, or replaces the team copy of it, and answers the team's view. */
  share(routine: Routine, locals: ReadonlyMap<string, unknown>): RoutineView {
    const body = teamBody(routine);
    const row: Row = { routine: body, author: 'You', updatedAt: ago(0), approved: body, on: this.rows.get(routine.id)?.on ?? false, folder: this.rows.get(routine.id)?.folder ?? null };

    this.rows.set(routine.id, row);
    this.sharedFrom.set(routine.id, fakeSha(routine));

    return this.view(row, locals);
  }

  remove(id: string, sha256: string): void {
    const row = this.require(id, sha256);

    this.rows.delete(row.routine.id);
    this.sharedFrom.delete(id);
  }

  /** Removes every routine shared as "You", and answers their ids. */
  removeMine(): string[] {
    const ids = [...this.rows.values()].filter((row) => row.author === 'You').map((row) => row.routine.id);

    for (const id of ids) {
      this.rows.delete(id);
      this.sharedFrom.delete(id);
    }

    return ids;
  }

  private require(id: string, sha256: string): Row {
    const row = this.rows.get(id);

    if (!row) {
      throw IpcFailure.from({ code: 'notFound', message: `team routine ${id} was not found` });
    }

    if (fakeSha(row.routine) !== sha256) {
      throw IpcFailure.from({ code: 'conflict', message: 'the remote routine changed since it was read' });
    }

    return row;
  }

  /** The team routine as a local, approved one with the same id. */
  copyOf(id: string, sha256: string, project: string | null): Routine {
    const row = this.require(id, sha256);

    return { ...row.routine, project: row.routine.scope === 'project' ? (project ?? ROUTINES_PROJECT) : null, createdBy: 'desktop', updatedBy: 'desktop', updatedAt: ago(0) };
  }

  /** Turns a team routine on for this computer, or off. */
  enable(id: string, enabled: boolean, project: string | null, locals: ReadonlyMap<string, unknown>): RoutineView {
    const row = this.rows.get(id);

    if (!row) {
      throw IpcFailure.from({ code: 'notFound', message: `team routine ${id} was not found` });
    }

    row.on = enabled;
    row.folder = enabled && row.routine.scope === 'project' ? (project ?? ROUTINES_PROJECT) : null;

    return this.view(row, locals);
  }

  /** Approves the exact team version, for a routine that asks for it. */
  approve(id: string, sha256: string, locals: ReadonlyMap<string, unknown>): RoutineView | null {
    if (!this.rows.has(id)) {
      return null;
    }

    const row = this.require(id, sha256);

    row.approved = row.routine;

    return this.view(row, locals);
  }

  private statusOf(row: Row, locals: ReadonlyMap<string, unknown>): RoutineStatus {
    const trusted = this.mode?.trust === true;

    if (locals.has(row.routine.id)) {
      return 'duplicate';
    }

    if (!row.on || !row.routine.enabled) {
      return 'off';
    }

    return trusted || row.approved !== null ? 'active' : 'pending';
  }

  view(row: Row, locals: ReadonlyMap<string, unknown>): RoutineView {
    const status = this.statusOf(row, locals);
    const action = row.routine.action;
    const original = this.sharedFrom.get(row.routine.id);
    const local = locals.get(row.routine.id) as { routine: Routine } | undefined;

    return {
      id: row.routine.id,
      routine: row.routine,
      name: row.routine.name,
      status,
      sha256: fakeSha(row.routine),
      problem: null,
      approval: row.approved === null ? 'never' : row.approved === row.routine ? 'approved' : 'changed',
      approved: row.approved !== row.routine ? row.approved : null,
      origin: 'team',
      runsUnattended: action.type === 'agent' && (action.approvalMode === 'yolo' || action.trustOnce),
      canRunNow: status === 'active',
      queued: false,
      nextRun: null,
      teamAuthor: row.author,
      teamUpdatedAt: row.updatedAt,
      teamRepo: row.routine.scope === 'project' ? ROUTINES_TEAM_REPO : null,
      teamChanged: original !== undefined && local !== undefined && fakeSha(local.routine) !== original,
      teamOn: row.on,
      teamFolder: row.folder,
      shareBlock: null,
    };
  }

  views(locals: ReadonlyMap<string, unknown>): RoutineView[] {
    return [...this.rows.values()].map((row) => this.view(row, locals));
  }

  /** Marks the routines shared before: `sharedNow` is the local version, changed since it went to the team as `sharedThen`. */
  sharedBefore(sharedThen: Routine, sharedNow: Routine): void {
    this.rows.set(sharedThen.id, { routine: teamBody(sharedThen), author: 'You', updatedAt: ago(8 * DAY), approved: teamBody(sharedThen), on: false, folder: null });
    this.sharedFrom.set(sharedNow.id, fakeSha(sharedThen));
  }
}
