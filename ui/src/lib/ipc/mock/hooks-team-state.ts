import type { Hook } from '../bindings/Hook';
import type { HookStatus } from '../bindings/HookStatus';
import type { HookView } from '../bindings/HookView';
import type { TeamHooks } from '../bindings/TeamHooks';
import { IpcFailure } from '../transport';
import { fakeSha, HOOKS_PROJECT } from './hooks-fixtures';
import { DEMO_REPO, REMOTE_ONLY_REPO, teamPlace } from './project-scopes';
import type { TeamMode } from './team-preview';

/** The repository the sample project hooks of the team belong to: only this key travels, never a folder. */
export const TEAM_REPO = DEMO_REPO;

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

function ago(ms: number): string {
  return new Date(Date.now() - ms).toISOString().replace(/\.\d+Z$/, 'Z');
}

/** The fields every local view has for the team: none of them apply. A global hook has no root or repository either (a project hook's come from `localPlace`). */
export const LOCAL_ORIGIN = { origin: 'local', teamAuthor: null, teamUpdatedAt: null, teamRepo: null, teamChanged: false, teamDisabled: false, root: null, repoKey: null } as const;

/** One hook on the team database. */
interface Row {
  hook: Hook;
  author: string;
  updatedAt: string;
  /** The version this computer approved; null when none was. */
  approved: Hook | null;
  /** The local off switch. */
  disabled: boolean;
}

/** The local hooks the team's rows are looked up against: a local hook with the same id overrides the team copy. */
export type LocalHooks = ReadonlyMap<string, { hook: Hook }>;

/** A hook as it is stored on the team: a project hook carries its repository key, not a folder. */
export function teamBody(hook: Hook): Hook {
  return { ...hook, project: hook.scope === 'project' ? TEAM_REPO : null };
}

function sample(id: string, patch: Partial<Hook> & Pick<Hook, 'name'>, age: number): Hook {
  return {
    id,
    description: null,
    scope: 'global',
    project: null,
    event: 'sessionStart',
    matcher: null,
    action: { type: 'addContext', text: '' },
    enabled: true,
    requires: [],
    seed: null,
    createdAt: ago(age),
    createdBy: 'desktop',
    updatedAt: ago(age),
    updatedBy: 'desktop',
    ...patch,
  };
}

const TYPES_ID = '7'.repeat(32);
const BODY_ID = '8'.repeat(32);
const COMMITS_ID = '9'.repeat(32);
const STATUS_ID = 'a'.repeat(32);

/** The team hooks of the previews, in the version this computer approved where it did. */
function sampleRows(): Row[] {
  const commits = sample(COMMITS_ID, { name: 'Use conventional commits', action: { type: 'addContext', text: 'Write commit messages as type(scope): summary, for example fix(ui): keep the focus on the row.' } }, 20 * 24 * HOUR);
  const typesOld = sample(TYPES_ID, { name: 'Run the type checker', scope: 'project', project: TEAM_REPO, event: 'toolEnd', matcher: 'edit', action: { type: 'command', command: 'bun run check', timeoutSecs: null } }, 12 * 24 * HOUR);
  const typesNow = { ...typesOld, matcher: 'edit|write', action: { type: 'command', command: 'bun run check && bun run lint', timeoutSecs: 30 } as const, updatedAt: ago(3 * HOUR), updatedBy: 'desktop' as const };
  const status = sample(STATUS_ID, { name: 'Show the git status when a chat ends', event: 'sessionEnd', action: { type: 'command', command: 'git status --short', timeoutSecs: null } }, 2 * HOUR);
  const styles = sample('c'.repeat(32), { name: 'Lint the stylesheets', scope: 'project', project: REMOTE_ONLY_REPO, event: 'toolEnd', matcher: 'edit', action: { type: 'command', command: 'bun run lint:css', timeoutSecs: null } }, 6 * 24 * HOUR);

  return [
    { hook: commits, author: 'Anna Kowalska', updatedAt: ago(20 * 24 * HOUR), approved: commits, disabled: false },
    { hook: typesNow, author: 'Marek Nowak', updatedAt: ago(3 * HOUR), approved: typesOld, disabled: false },
    { hook: status, author: 'Anna Kowalska', updatedAt: ago(2 * HOUR), approved: null, disabled: false },
    { hook: styles, author: 'Marek Nowak', updatedAt: ago(6 * 24 * HOUR), approved: styles, disabled: false },
  ];
}

/** The local sample hooks a team preview adds: one the user shared before and changed since, and one still to share. */
export function sharedSamples(): { shared: Hook; sharedNow: Hook; unshared: Hook } {
  const shared = sample(BODY_ID, { name: 'Keep commits small', event: 'turnEnd', action: { type: 'followUp', text: 'If this turn touched more than a few files, suggest splitting the commit.' } }, 8 * 24 * HOUR);
  const sharedNow = { ...shared, action: { type: 'followUp', text: 'If this turn touched more than a few files, suggest splitting the commit and say how.' } as const, updatedAt: ago(30 * MINUTE) };
  const unshared = sample('b'.repeat(32), { name: 'Summarize the diff', event: 'turnEnd', action: { type: 'followUp', text: 'End with a short summary of what changed and why.' } }, 5 * 24 * HOUR);

  return { shared, sharedNow, unshared };
}

/** The team hooks of the mock backend, kept in memory: the cache the real desktop keeps of the team database, and what it approved. */
export class MockTeam {
  private rows = new Map<string, Row>();
  /** The local version each hook was shared from, by id. */
  private sharedFrom = new Map<string, string>();
  private refreshedAt = ago(MINUTE);

  constructor(readonly mode: TeamMode | null) {
    if (mode) {
      for (const row of sampleRows()) {
        this.rows.set(row.hook.id, row);
      }
    }
  }

  /** Records that these local hooks were shared before: `shared` is the version that went to the team, `reminder` is as it went. */
  sharedBefore(shared: Hook, reminder: Hook): void {
    this.rows.set(shared.id, { hook: teamBody(shared), author: 'You', updatedAt: ago(8 * 24 * HOUR), approved: teamBody(shared), disabled: false });
    this.rows.set(reminder.id, { hook: teamBody(reminder), author: 'You', updatedAt: ago(2 * 24 * HOUR), approved: teamBody(reminder), disabled: false });
    this.sharedFrom.set(shared.id, fakeSha(shared));
    this.sharedFrom.set(reminder.id, fakeSha(reminder));
  }

  get on(): boolean {
    return this.mode !== null;
  }

  has(id: string): boolean {
    return this.rows.has(id);
  }

  info(): TeamHooks | null {
    return this.mode ? { trust: this.mode.trust, problem: this.mode.problem, refreshedAt: this.refreshedAt } : null;
  }

  /** The team's views for the project: the global ones, and the repository's when the project has an origin remote. Global first. With `all` every team hook, of any repository. */
  views(local: LocalHooks, project: string | null, all = false): HookView[] {
    const ofProject = (hook: Hook) => project !== null && this.mode?.shareable === true && hook.project === TEAM_REPO;
    const visible = [...this.rows.values()].filter(({ hook }) => all || hook.scope === 'global' || ofProject(hook));
    const order = ({ hook }: Row) => `${hook.scope === 'global' ? 0 : 1}${hook.createdAt}${hook.id}`;

    return visible.sort((a, b) => order(a).localeCompare(order(b))).map((row) => this.view(row, local));
  }

  private statusOf(row: Row, local: LocalHooks): HookStatus {
    if (local.has(row.hook.id)) {
      return 'duplicate';
    }

    if (!this.mode?.trust && (row.approved === null || fakeSha(row.approved) !== fakeSha(row.hook))) {
      return 'pending';
    }

    return row.hook.enabled && !row.disabled ? 'active' : 'off';
  }

  private view(row: Row, local: LocalHooks): HookView {
    const { hook } = row;
    const status = this.statusOf(row, local);
    const sharedSha = this.sharedFrom.get(hook.id);
    const original = local.get(hook.id)?.hook;
    const changedApproval = this.mode?.trust !== true && row.approved !== null && status === 'pending';

    return {
      id: hook.id,
      hook,
      name: hook.name,
      status,
      sha256: fakeSha(hook),
      problem: null,
      approval: this.mode?.trust ? 'approved' : row.approved === null ? 'never' : changedApproval ? 'changed' : 'approved',
      approved: changedApproval ? row.approved : null,
      origin: 'team',
      teamAuthor: row.author,
      teamUpdatedAt: row.updatedAt,
      teamRepo: hook.scope === 'project' ? hook.project : null,
      teamChanged: original !== undefined && sharedSha !== undefined && fakeSha(original) !== sharedSha,
      shareBlock: null,
      teamDisabled: row.disabled,
      ...teamPlace(hook.scope === 'project' ? hook.project : null),
    };
  }

  private require(id: string, sha256: string): Row {
    const row = this.rows.get(id);

    if (!row) {
      throw IpcFailure.from({ code: 'notFound', message: `team hook ${id} was not found` });
    }

    if (fakeSha(row.hook) !== sha256) {
      throw IpcFailure.from({ code: 'conflict', message: 'the remote copy changed since it was read' });
    }

    return row;
  }

  /** Approves exactly this version of a team hook, when it is one. Answers null for an id the team does not have. */
  approve(id: string, sha256: string, local: LocalHooks): HookView | null {
    if (!this.rows.has(id)) {
      return null;
    }

    const row = this.require(id, sha256);

    row.approved = row.hook;

    return this.view(row, local);
  }

  /** Puts a local hook on the team, or its new version in place of the team copy (the row keeps its author). */
  share(hook: Hook, local: LocalHooks): HookView {
    if (!this.mode) {
      throw IpcFailure.from({ code: 'unsupported', message: 'hooks are not shared with a team' });
    }

    if (hook.seed !== null) {
      throw IpcFailure.from({ code: 'forbidden', message: 'Harness family hooks are never shared' });
    }

    if (hook.scope === 'project' && !this.mode.shareable) {
      throw IpcFailure.from({ code: 'forbidden', message: 'this repository has no origin remote, so its hooks cannot be shared' });
    }

    const row: Row = { hook: teamBody(hook), author: this.rows.get(hook.id)?.author ?? 'You', updatedAt: ago(0), approved: teamBody(hook), disabled: this.rows.get(hook.id)?.disabled ?? false };

    this.rows.set(hook.id, row);
    this.sharedFrom.set(hook.id, fakeSha(hook));

    return this.view(row, local);
  }

  remove(id: string, sha256: string): null {
    this.require(id, sha256);
    this.rows.delete(id);
    this.sharedFrom.delete(id);

    return null;
  }

  /** The local hook a team hook turns into: its own, approved, with the same id and the given folder for a project hook. */
  copyOf(id: string, sha256: string, project: string | null, local: LocalHooks): Hook {
    const row = this.require(id, sha256);
    const { hook } = row;

    // Like the backend: a version nobody approved on this computer is never copied.
    if (this.statusOf(row, local) === 'pending') {
      throw IpcFailure.from({ code: 'invalidInput', message: 'this team hook waits for your approval and cannot be copied' });
    }

    if (local.has(id)) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'a hook of yours already has this id' });
    }

    if (hook.scope === 'project' && project === null) {
      throw IpcFailure.from({ code: 'invalidInput', message: 'open a folder of this repository to copy a project hook' });
    }

    return { ...hook, project: hook.scope === 'project' ? (project ?? HOOKS_PROJECT) : null, createdBy: 'desktop', updatedBy: 'desktop' };
  }

  enable(id: string, enabled: boolean, local: LocalHooks): HookView {
    const row = this.rows.get(id);

    if (!row) {
      throw IpcFailure.from({ code: 'notFound', message: `team hook ${id} was not found` });
    }

    row.disabled = !enabled;

    return this.view(row, local);
  }

  /** The stand-in for reading the team database again. */
  refresh(): void {
    this.refreshedAt = ago(0);
  }
}
