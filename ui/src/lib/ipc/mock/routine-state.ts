import type { AgentSessionInfo } from '../bindings/AgentSessionInfo';
import type { HookApproval } from '../bindings/HookApproval';
import type { Routine } from '../bindings/Routine';
import type { RoutineCounts } from '../bindings/RoutineCounts';
import type { RoutineDraft } from '../bindings/RoutineDraft';
import type { RoutineListing } from '../bindings/RoutineListing';
import type { RoutineRun } from '../bindings/RoutineRun';
import type { RoutineRunsPage } from '../bindings/RoutineRunsPage';
import type { RoutineRunsQuery } from '../bindings/RoutineRunsQuery';
import type { RoutinesDeleted } from '../bindings/RoutinesDeleted';
import type { RoutineStatus } from '../bindings/RoutineStatus';
import type { RoutinesEvent } from '../bindings/RoutinesEvent';
import type { SessionRoutineTag } from '../bindings/SessionRoutineTag';
import type { RoutineView } from '../bindings/RoutineView';
import type { TeamRoutinesRemoved } from '../bindings/TeamRoutinesRemoved';
import { IpcFailure } from '../transport';
import { fixtures } from './fixtures';
import { previewSchedule } from './routine-schedule';
import { ago, fakeSha, liveRun, routineId, ROUTINES_PROJECT, ROUTINES_WORKSPACE, sample, sampleRoutines, sampleRuns } from './routine-fixtures';
import { MockRuns } from './routine-runs-state';
import { LOCAL_ORIGIN, MockRoutineTeam } from './routine-team-state';
import { teamModeOf } from './team-preview';

/** Browser-preview switch `?routines=`: what the Routines view finds. Anything else shows the sample routines. */
export const ROUTINE_SCENARIOS = ['ok', 'empty', 'invalid', 'problem', 'failing', 'team', 'teamTrusted', 'teamNoOrigin', 'teamProblem'] as const;
export type RoutineScenario = (typeof ROUTINE_SCENARIOS)[number];

export function routineScenarioFromLocation(search: string = globalThis.location?.search ?? ''): RoutineScenario {
  const name = new URLSearchParams(search).get('routines');

  return ROUTINE_SCENARIOS.find((scenario) => scenario === name) ?? 'ok';
}

/** Where a routine file stands in the approvals. */
interface Stored {
  routine: Routine;
  /** The last bytes the user approved; null when none ever were. */
  approved: Routine | null;
  /** The file was rewritten after the approval. */
  changed: boolean;
}

const MAX_PER_SCOPE = 64;

/**
 * Whether a routine belongs to the folders a list or a delete is asked for: the global ones, and the project's own. The sample
 * project routines also belong to whatever project a browser preview opens, so the preview shows them.
 */
function inScope(routine: Routine, project: string | null): boolean {
  return routine.scope === 'global' || (project !== null && (routine.project === project || routine.project === ROUTINES_PROJECT));
}

/** The number of finished runs the sample history holds: more than two pages. */
const HISTORY = 120;

function now(): string {
  return new Date().toISOString().replace(/\.\d+Z$/, 'Z');
}

/** The routines folder, the approvals, the run history and the team of the mock backend, kept in memory. */
export class MockRoutines {
  private files = new Map<string, Stored>();
  private invalid = new Map<string, string>();
  private readonly runs = new MockRuns((event) => this.emit(event));
  private listener: ((event: RoutinesEvent) => void) | null = null;
  private readonly team: MockRoutineTeam;

  constructor(private readonly scenario: () => RoutineScenario) {
    this.team = new MockRoutineTeam(teamModeOf(scenario()));
    this.seed(scenario());
  }

  private seed(scenario: RoutineScenario): void {
    if (scenario === 'empty') {
      return;
    }

    const { active, pending, changedApproved, changed } = sampleRoutines();

    for (const routine of active) {
      this.put(routine);
    }

    this.files.set(pending.id, { routine: pending, approved: null, changed: false });
    this.files.set(changed.id, { routine: changed, approved: changedApproved, changed: true });

    const sources = [...active, changedApproved].map((routine) => ({ id: routine.id, name: routine.name, kind: routine.action.type, project: routine.project }));
    const [, , , tests, triage] = sources;

    this.runs.seed(sampleRuns(sources, HISTORY), [liveRun(tests, false, 40_000), liveRun(triage, true, 6 * 60_000)]);

    if (this.team.on) {
      const shared = sample({ name: 'Keep commits small', when: { type: 'daily', at: '17:00' }, action: { type: 'agent', harness: 'omp', model: null, prompt: 'If today\'s work touched more than a few files, suggest how to split it.', approvalMode: null, trustOnce: false, maxMinutes: 10 } }, 8 * 24 * 3_600_000);
      const sharedNow: Routine = { ...shared, name: 'Keep commits small (evening)', updatedAt: ago(30 * 60_000) };

      this.put(sharedNow);
      this.team.sharedBefore(shared, sharedNow);
    }

    if (scenario === 'invalid') {
      this.invalid.set(routineId(), 'The cron expression `0 0 31 2 *` never matches a day.');
    }
  }

  private put(routine: Routine): Stored {
    const stored: Stored = { routine, approved: routine, changed: false };

    this.files.set(routine.id, stored);

    return stored;
  }

  private emit(event: RoutinesEvent): void {
    queueMicrotask(() => this.listener?.(event));
  }

  private notify(): void {
    this.emit({ kind: 'changed' });
  }

  private require(id: string, sha256: string): Stored {
    const stored = this.files.get(id);

    if (!stored) {
      throw IpcFailure.from({ code: 'notFound', message: `routine ${id} was not found` });
    }

    if (fakeSha(stored.routine) !== sha256) {
      throw IpcFailure.from({ code: 'conflict', message: 'the routine changed since it was read' });
    }

    return stored;
  }

  private approvalOf(stored: Stored): HookApproval {
    if (stored.approved === null) {
      return 'never';
    }

    return stored.changed ? 'changed' : 'approved';
  }

  private statusOf(stored: Stored): RoutineStatus {
    if (stored.changed || stored.approved === null) {
      return 'pending';
    }

    if (this.runs.isRunning(stored.routine.id)) {
      return 'running';
    }

    return stored.routine.enabled ? 'active' : 'off';
  }

  view(stored: Stored): RoutineView {
    const { routine } = stored;
    const status = this.statusOf(stored);
    const action = routine.action;
    const scheduled = status === 'active' || (status === 'running' && routine.enabled);

    return {
      id: routine.id,
      routine,
      name: routine.name,
      status,
      sha256: fakeSha(routine),
      problem: null,
      approval: this.approvalOf(stored),
      approved: stored.changed ? stored.approved : null,
      ...LOCAL_ORIGIN,
      runsUnattended: action.type === 'agent' && (action.approvalMode === 'yolo' || action.trustOnce),
      canRunNow: status !== 'pending',
      queued: false,
      nextRun: scheduled ? (previewSchedule(routine.when, new Date(), 1).next[0] ?? null) : null,
      shareBlock: this.team.on ? this.team.shareBlock(routine, status === 'pending') : null,
    };
  }

  private invalidView(id: string, problem: string): RoutineView {
    return { ...this.blank(id), problem };
  }

  private blank(id: string): RoutineView {
    return {
      id,
      routine: null,
      name: null,
      status: 'invalid',
      sha256: fakeSha(id),
      problem: null,
      approval: 'never',
      approved: null,
      ...LOCAL_ORIGIN,
      runsUnattended: false,
      canRunNow: false,
      queued: false,
      nextRun: null,
      shareBlock: null,
    };
  }

  /** The routines of the global scope and of `project`, valid ones first, and the team's after them. */
  list(project: string | null): RoutineListing {
    if (this.scenario() === 'failing') {
      throw IpcFailure.from({ code: 'internal', message: 'The routines folder could not be read.' });
    }

    if (this.scenario() === 'problem') {
      return { routines: [], folderProblem: 'The folder is writable by other users.', team: null, counts: countsOf([]) };
    }

    const locals = [...this.files.values()].filter(({ routine }) => inScope(routine, project));
    const order = (stored: Stored): number => (stored.routine.scope === 'global' ? 0 : 1);
    const valid = locals.sort((a, b) => order(a) - order(b) || a.routine.createdAt.localeCompare(b.routine.createdAt)).map((stored) => this.view(stored));
    const broken = [...this.invalid].map(([id, problem]) => this.invalidView(id, problem));
    const team = this.team.on ? this.team.views(this.files) : [];
    const routines = [...valid, ...broken, ...team];

    return { routines, folderProblem: null, team: this.team.state(), counts: countsOf(routines) };
  }

  /** Creates or edits a routine the way the desktop does: the written version is approved at once. */
  save(draft: RoutineDraft): RoutineView {
    const existing = draft.id === null ? null : this.require(draft.id, draft.expectedSha256 ?? '');

    // A save approves what it writes, so the real backend refuses it for a routine that waits for the user's approval.
    if (existing && this.statusOf(existing) === 'pending') {
      throw IpcFailure.from({ code: 'invalidInput', message: 'this routine waits for your approval and cannot be saved' });
    }

    if (draft.name.trim() === '') {
      throw IpcFailure.from({ code: 'invalidInput', message: 'a routine needs a name' });
    }

    const project = draft.scope === 'project' ? (draft.project ?? ROUTINES_PROJECT) : null;
    const held = [...this.files.values()].filter(({ routine }) => routine.scope === draft.scope && routine.project === project).length;

    if (!existing && held >= MAX_PER_SCOPE) {
      throw IpcFailure.from({ code: 'invalidInput', message: `a scope holds at most ${MAX_PER_SCOPE} routines` });
    }

    const routine: Routine = {
      id: existing?.routine.id ?? routineId(),
      name: draft.name.trim(),
      description: draft.description,
      scope: draft.scope,
      project,
      when: draft.when,
      action: draft.action,
      enabled: draft.enabled,
      createdAt: existing?.routine.createdAt ?? now(),
      createdBy: existing?.routine.createdBy ?? 'desktop',
      updatedAt: now(),
      updatedBy: 'desktop',
    };

    this.files.set(routine.id, { routine, approved: routine, changed: false });
    this.notify();

    return this.view(this.files.get(routine.id) as Stored);
  }

  delete(id: string, sha256: string): null {
    this.require(id, sha256);
    this.files.delete(id);
    this.invalid.delete(id);
    this.runs.dropRunsOf(id);
    this.notify();

    return null;
  }

  /** Deletes the global routines and the ones of `project`, with their approvals and runs, and the files that break the format too. The count includes those files. */
  deleteAll(project: string | null): RoutinesDeleted {
    const gone = [...this.files.values()].filter(({ routine }) => inScope(routine, project));

    for (const { routine } of gone) {
      this.files.delete(routine.id);
      this.runs.dropRunsOf(routine.id);
    }

    const broken = this.invalid.size;

    this.invalid.clear();
    this.notify();

    return { deleted: gone.length + broken };
  }

  /** Approves a local routine, or a team routine that asks for it. */
  approve(id: string, sha256: string): RoutineView {
    const local = this.files.get(id);
    const asked = local ? null : this.team.approve(id, sha256, this.files);

    if (asked) {
      this.notify();

      return asked;
    }

    const stored = this.require(id, sha256);

    stored.approved = stored.routine;
    stored.changed = false;
    this.notify();

    return this.view(stored);
  }

  /** A change goes back to the approved version, a routine never approved goes away. */
  reject(id: string, sha256: string): RoutineView | null {
    const stored = this.require(id, sha256);

    if (!stored.approved) {
      this.files.delete(id);
      this.notify();

      return null;
    }

    stored.routine = stored.approved;
    stored.changed = false;
    this.notify();

    return this.view(stored);
  }

  /** Starts a run of an approved routine. It ends by itself a moment later. */
  runNow(id: string): RoutineRun {
    const stored = this.files.get(id);

    if (!stored) {
      throw IpcFailure.from({ code: 'notFound', message: `routine ${id} was not found` });
    }

    if (this.statusOf(stored) === 'pending') {
      throw IpcFailure.from({ code: 'invalidInput', message: 'a routine that waits for approval does not run' });
    }

    if (this.runs.isRunning(id)) {
      throw IpcFailure.from({ code: 'conflict', message: 'this routine is already running' });
    }

    return this.runs.start({ id, name: stored.routine.name, kind: stored.routine.action.type, project: stored.routine.project });
  }

  stop(runId: string): null {
    return this.runs.stop(runId);
  }

  /** The question of a run is answered: the run stops waiting. */
  answer(runId: string): void {
    this.runs.answer(runId);
  }

  runsPage(query: RoutineRunsQuery): RoutineRunsPage {
    return this.runs.page(query);
  }

  /** The chat of a run that has one: a session of the run's harness, in the run's folder. */
  attach(runId: string): AgentSessionInfo {
    const run = this.runs.all().find((candidate) => candidate.id === runId);

    if (!run?.session) {
      throw IpcFailure.from({ code: 'notFound', message: 'this run has no chat' });
    }

    const info = structuredClone(fixtures.agent.sessionInfo[run.session.harness]);

    return { ...info, key: `routine-${run.id}`, project: run.project ?? `${ROUTINES_WORKSPACE}/${run.routineId}`, startedAt: run.startedAt };
  }

  /** What a run's chat says: the prompt of its routine (a run is shown from the start), and whether the run waits for an answer. */
  conversation(runId: string): { prompt: string; asks: boolean } {
    const run = this.runs.all().find((candidate) => candidate.id === runId);
    const action = run ? this.files.get(run.routineId)?.routine.action : undefined;

    return { prompt: action?.type === 'agent' ? action.prompt : 'Run the routine.', asks: run?.needsYou ?? false };
  }

  /**
   * The routine tags the Sessions preview puts on its sessions: the agent runs that left a session, the ones going on first,
   * split by whether their routine is global (its sessions are in the routines' own folder) or belongs to a project.
   */
  sessionTags(): { project: SessionRoutineTag[]; global: SessionRoutineTag[] } {
    const tagged = this.runs.all().filter((run) => run.session !== null);
    const tags = (global: boolean) => tagged.filter((run) => (run.project === null) === global).map((run) => ({ routineId: run.routineId, runId: run.id, name: run.routineName }));

    return { project: tags(false), global: this.workspace() === null ? [] : tags(true) };
  }

  /** The folder global routines run in, once a global routine has run. */
  workspace(): string | null {
    return this.scenario() === 'empty' ? null : ROUTINES_WORKSPACE;
  }

  listen(onEvent: (event: RoutinesEvent) => void): null {
    this.listener = onEvent;

    return null;
  }

  get(id: string): RoutineView {
    const stored = this.files.get(id);

    if (!stored) {
      throw IpcFailure.from({ code: 'notFound', message: `routine ${id} was not found` });
    }

    return this.view(stored);
  }

  /** Shares the local routine at this version with the team, or updates the team copy of it, and answers the team's view. */
  share(id: string, sha256: string): RoutineView {
    const stored = this.require(id, sha256);
    const shared = this.team.share(stored.routine, this.files);

    this.notify();

    return shared;
  }

  teamRemove(id: string, sha256: string): null {
    this.team.remove(id, sha256);
    this.notify();

    return null;
  }

  teamRemoveMine(): TeamRoutinesRemoved {
    const ids = this.team.removeMine();

    this.notify();

    return { deleted: ids.length, ids };
  }

  /** Reads the team database again (here: stamps the time), then lists what `routines_list` lists. */
  teamRefresh(project: string | null): RoutineListing {
    this.team.refresh();

    return this.list(project);
  }

  /** Turns the team routine into a local, approved one with the same id. */
  teamCopy(id: string, sha256: string, project: string | null): RoutineView {
    this.put(this.team.copyOf(id, sha256, project));
    this.notify();

    return this.get(id);
  }

  teamEnable(id: string, enabled: boolean, project: string | null): RoutineView {
    const view = this.team.enable(id, enabled, project, this.files);

    this.notify();

    return view;
  }

  /** Somebody else changes the file of a routine, as an editor or a script would: its bytes change, so does its version. */
  rewrite(id: string, patch: Partial<Routine>): void {
    const stored = this.files.get(id);

    if (stored) {
      stored.routine = { ...stored.routine, ...patch, updatedAt: now() };
    }
  }

  /** An agent writes a new routine through the Harness Desktop MCP: the file is there, and nobody approved it. */
  agentAdds(name: string): RoutineView {
    const routine: Routine = { ...sample({ name, when: { type: 'daily', at: '18:00' }, action: { type: 'command', command: 'git status --short', timeoutSecs: null } }, 0), createdBy: 'agent', updatedBy: 'agent' };

    this.files.set(routine.id, { routine, approved: null, changed: false });
    this.notify();

    return this.view(this.files.get(routine.id) as Stored);
  }
}

/** The counts of each filter value over every listed routine. */
function countsOf(routines: RoutineView[]): RoutineCounts {
  const counts: RoutineCounts = { total: routines.length, status: {}, when: {}, origin: {} };

  for (const view of routines) {
    counts.status[view.status] = (counts.status[view.status] ?? 0) + 1;
    counts.origin[view.origin] = (counts.origin[view.origin] ?? 0) + 1;

    if (view.routine) {
      counts.when[view.routine.when.type] = (counts.when[view.routine.when.type] ?? 0) + 1;
    }
  }

  return counts;
}
