import type { Hook } from '../bindings/Hook';
import type { HookApproval } from '../bindings/HookApproval';
import type { HookDraft } from '../bindings/HookDraft';
import type { HookFamilyEntry } from '../bindings/HookFamilyEntry';
import type { HookListing } from '../bindings/HookListing';
import type { HookRequirement } from '../bindings/HookRequirement';
import type { HookSeed } from '../bindings/HookSeed';
import type { HookStatus } from '../bindings/HookStatus';
import type { HookView } from '../bindings/HookView';
import type { ShareBlock } from '../bindings/ShareBlock';
import { IpcFailure } from '../transport';
import { CATALOG, type CatalogEntry, fakeSha, familyHook, hookId, HOOKS_PROJECT, lintApproved, lintChanged, EDITED_SAVE_TEXT, OLD_SAVE_TEXT, testsReminder } from './hooks-fixtures';
import { LOCAL_ORIGIN, MockTeam, sharedSamples } from './hooks-team-state';
import { localPlace } from './project-scopes';
import { teamModeOf } from './team-preview';

/** Browser-preview switch `?hooks=`: what the Hooks view finds. Anything else shows the sample hooks. */
export const HOOK_SCENARIOS = ['ok', 'empty', 'outdated', 'outdatedEdited', 'noDashboard', 'invalid', 'problem', 'failing', 'team', 'teamTrusted', 'teamNoOrigin', 'teamProblem'] as const;
export type HookScenario = (typeof HOOK_SCENARIOS)[number];

export function hookScenarioFromLocation(search: string = globalThis.location?.search ?? ''): HookScenario {
  const name = new URLSearchParams(search).get('hooks');

  return HOOK_SCENARIOS.find((scenario) => scenario === name) ?? 'ok';
}

/** Where a hook file stands in the approvals. */
interface Stored {
  hook: Hook;
  /** The last bytes the user approved; null when none ever were. */
  approved: Hook | null;
  /** The file was rewritten after the approval. */
  changed: boolean;
}

const MAX_PER_SCOPE = 64;

/** The text the memory-save hook holds in the scenarios where it is outdated: an earlier catalog version, or one the user edited. */
const SAVE_TEXTS: Partial<Record<HookScenario, string>> = { outdated: OLD_SAVE_TEXT, outdatedEdited: EDITED_SAVE_TEXT };

/** The hooks folder, the approvals and the family catalog of the mock backend, kept in memory. */
export class MockHooks {
  private files = new Map<string, Stored>();
  private invalid = new Map<string, string>();
  private listener: (() => void) | null = null;
  private installed: Record<HookRequirement, boolean> = { 'harness-memory': true, 'harness-dashboard': true };
  private readonly team: MockTeam;

  constructor(
    private readonly scenario: () => HookScenario,
    /** Whether the memory setting is on, which the memory family hooks wait for. */
    private readonly memoryOn: () => boolean,
  ) {
    this.team = new MockTeam(teamModeOf(scenario()));
    this.seed(scenario());
  }

  private seed(scenario: HookScenario): void {
    if (scenario === 'empty') {
      return;
    }

    const [search, save] = CATALOG;
    const lint = lintApproved(hookId());

    this.put(familyHook(search));
    this.put(familyHook(save, SAVE_TEXTS[scenario] ?? save.text));
    this.files.set(lint.id, { hook: lintChanged(lint), approved: lint, changed: true });
    const reminder = this.put(testsReminder()).hook;

    // A team preview has hooks the user shared before: one changed since, one as it went, and one not shared yet.
    if (this.team.on) {
      const { shared, sharedNow, unshared } = sharedSamples();

      this.put(sharedNow);
      this.put(unshared);
      this.team.sharedBefore(shared, reminder);
    }

    if (scenario === 'noDashboard') {
      this.installed['harness-dashboard'] = false;
    }

    if (scenario === 'invalid') {
      this.invalid.set(hookId(), 'The action `followUp` cannot run at `sessionStart`.');
    }
  }

  private put(hook: Hook): Stored {
    const stored: Stored = { hook, approved: hook, changed: false };

    this.files.set(hook.id, stored);

    return stored;
  }

  private notify(): void {
    queueMicrotask(() => this.listener?.());
  }

  private require(id: string, sha256: string): Stored {
    const stored = this.files.get(id);

    if (!stored) {
      throw IpcFailure.from({ code: 'notFound', message: `hook ${id} was not found` });
    }

    if (fakeSha(stored.hook) !== sha256) {
      throw IpcFailure.from({ code: 'conflict', message: 'the hook changed since it was read' });
    }

    return stored;
  }

  private statusOf(stored: Stored): HookStatus {
    const { hook } = stored;

    if (stored.changed || stored.approved === null) {
      return 'pending';
    }

    if (!hook.enabled) {
      return 'off';
    }

    return hook.requires.every((requirement) => this.met(requirement)) ? 'active' : 'paused';
  }

  private met(requirement: HookRequirement): boolean {
    return requirement === 'harness-memory' ? this.installed[requirement] && this.memoryOn() : this.installed[requirement];
  }

  /** Why a hook cannot be shared with the team, in the order the backend checks. Nothing while hooks are not shared. */
  private shareBlock(stored: Stored): ShareBlock | null {
    const { hook } = stored;

    if (!this.team.on) {
      return null;
    }

    if (hook.seed !== null) {
      return 'seed';
    }

    if (this.statusOf(stored) === 'pending') {
      return 'pending';
    }

    if (hook.scope === 'project' && !this.team.mode?.shareable) {
      return 'noOrigin';
    }

    return this.team.has(hook.id) ? 'onTeam' : null;
  }

  private view(stored: Stored): HookView {
    const approval: HookApproval = stored.approved === null ? 'never' : stored.changed ? 'changed' : 'approved';

    return {
      id: stored.hook.id,
      hook: stored.hook,
      name: stored.hook.name,
      status: this.statusOf(stored),
      sha256: fakeSha(stored.hook),
      problem: null,
      approval,
      approved: stored.changed ? stored.approved : null,
      ...LOCAL_ORIGIN,
      shareBlock: this.shareBlock(stored),
      ...localPlace(stored.hook, this.team.mode),
    };
  }

  /** Global hooks first, then by creation, as the real folder runs them; files that break the format come last. With `all` the hooks of every project and every team hook. */
  list(project: string | null, all = false): HookListing {
    if (this.scenario() === 'problem') {
      return { hooks: [], folderProblem: 'The folder can be changed by other users on this computer.', team: null };
    }

    if (this.scenario() === 'failing') {
      throw IpcFailure.from({ code: 'io', message: 'The hooks folder could not be read.' });
    }

    const visible = [...this.files.values()].filter(({ hook }) => all || hook.scope === 'global' || (project !== null && hook.project === project));
    const order = (hook: Hook) => `${hook.scope === 'global' ? 0 : 1}${hook.createdAt}${hook.id}`;
    const hooks = visible.sort((a, b) => order(a.hook).localeCompare(order(b.hook))).map((stored) => this.view(stored));
    const broken = [...this.invalid].map(([id, problem]): HookView => ({ id, hook: null, name: 'Format on save', status: 'invalid', sha256: fakeSha({ id } as Hook), problem, approval: 'never', approved: null, ...LOCAL_ORIGIN, shareBlock: this.team.on ? 'invalid' : null }));

    return { hooks: [...hooks, ...broken, ...this.team.views(this.files, project, all)], folderProblem: null, team: this.team.info() };
  }

  /** Creates or edits a hook the way the desktop does: the written version is approved at once. */
  save(draft: HookDraft): HookView {
    const existing = draft.id === null ? null : this.require(draft.id, draft.expectedSha256 ?? '');

    // A save approves what it writes, so the real backend refuses it for a hook that waits for the user's approval.
    if (existing && this.statusOf(existing) === 'pending') {
      throw IpcFailure.from({ code: 'invalidInput', message: 'this hook waits for your approval and cannot be saved' });
    }

    if (draft.name.trim() === '') {
      throw IpcFailure.from({ code: 'invalidInput', message: 'a hook needs a name' });
    }

    const project = draft.scope === 'project' ? (draft.project ?? HOOKS_PROJECT) : null;
    const inScope = [...this.files.values()].filter(({ hook }) => hook.scope === draft.scope && hook.project === project).length;

    if (!existing && inScope >= MAX_PER_SCOPE) {
      throw IpcFailure.from({ code: 'invalidInput', message: `a scope holds at most ${MAX_PER_SCOPE} hooks` });
    }

    const now = new Date().toISOString().replace(/\.\d+Z$/, 'Z');
    const hook: Hook = {
      id: existing?.hook.id ?? hookId(),
      name: draft.name.trim(),
      description: draft.description,
      scope: draft.scope,
      project,
      event: draft.event,
      matcher: draft.event === 'toolEnd' ? draft.matcher : null,
      action: draft.action,
      enabled: draft.enabled,
      requires: existing?.hook.requires ?? [],
      seed: existing?.hook.seed ?? null,
      createdAt: existing?.hook.createdAt ?? now,
      createdBy: existing?.hook.createdBy ?? 'desktop',
      updatedAt: now,
      updatedBy: 'desktop',
    };

    this.files.set(hook.id, { hook, approved: hook, changed: false });
    this.notify();

    return this.view(this.files.get(hook.id) as Stored);
  }

  delete(id: string, sha256: string): null {
    this.require(id, sha256);
    this.files.delete(id);
    this.invalid.delete(id);
    this.notify();

    return null;
  }

  /** Deletes the global hooks and the ones of `project`, with their approvals, and the files that break the format too. The count includes those files. */
  deleteAll(project: string | null): { deleted: number } {
    const gone = [...this.files.values()].filter(({ hook }) => hook.scope === 'global' || (project !== null && hook.project === project));

    for (const { hook } of gone) {
      this.files.delete(hook.id);
    }

    const broken = this.invalid.size;
    this.invalid.clear();
    this.notify();

    return { deleted: gone.length + broken };
  }

  /** Approves a local hook, or a team hook while "Trust team hooks" is off. */
  approve(id: string, sha256: string): HookView {
    if (!this.files.has(id)) {
      const team = this.team.approve(id, sha256, this.files);

      if (team) {
        this.notify();

        return team;
      }
    }

    const stored = this.require(id, sha256);

    stored.approved = stored.hook;
    stored.changed = false;
    this.notify();

    return this.view(stored);
  }

  /** A change goes back to the approved version, a hook never approved goes away. */
  reject(id: string, sha256: string): HookView | null {
    const stored = this.require(id, sha256);

    if (!stored.approved) {
      this.files.delete(id);
      this.notify();

      return null;
    }

    stored.hook = stored.approved;
    stored.changed = false;
    this.notify();

    return this.view(stored);
  }

  /** Shares the local hook at this version with the team, or updates the team copy of it. */
  share(id: string, sha256: string): HookView {
    const view = this.team.share(this.require(id, sha256).hook, this.files);

    this.notify();

    return view;
  }

  teamRemove(id: string, sha256: string): null {
    this.team.remove(id, sha256);
    this.notify();

    return null;
  }

  /** Reads the team database again (here: stamps the time), then lists what `hooks_list` lists. */
  teamRefresh(project: string | null): HookListing {
    this.team.refresh();

    return this.list(project);
  }

  /** Turns the team hook into a local, approved one with the same id. */
  teamCopy(id: string, sha256: string, project: string | null): HookView {
    const hook = this.team.copyOf(id, sha256, project, this.files);

    this.put(hook);
    this.notify();

    return this.view(this.files.get(id) as Stored);
  }

  teamEnable(id: string, enabled: boolean): HookView {
    const view = this.team.enable(id, enabled, this.files);

    this.notify();

    return view;
  }

  private entryStatus(entry: CatalogEntry): HookFamilyEntry['status'] {
    const found = [...this.files.values()].find(({ hook }) => hook.seed === entry.seed);

    if (!this.installed[entry.member]) {
      return 'needsMember';
    }

    if (!found) {
      return 'missing';
    }

    return found.hook.action.type !== 'command' && found.hook.action.text === entry.text ? 'installed' : 'outdated';
  }

  family(): HookFamilyEntry[] {
    return CATALOG.map((entry) => ({
      seed: entry.seed,
      member: entry.member,
      name: entry.name,
      event: entry.event,
      status: this.entryStatus(entry),
      hookId: [...this.files.values()].find(({ hook }) => hook.seed === entry.seed)?.hook.id ?? null,
      stale: this.isStale(entry),
    }));
  }

  /** An outdated family hook that still holds the earlier catalog text, word for word. */
  private isStale(entry: CatalogEntry): boolean {
    const found = [...this.files.values()].find(({ hook }) => hook.seed === entry.seed);

    return this.entryStatus(entry) === 'outdated' && found?.hook.action.type !== 'command' && found?.hook.action.text === OLD_SAVE_TEXT;
  }

  /** Adds the missing and updates the outdated family hooks asked for, approved. */
  applyFamily(seeds: HookSeed[]): HookListing {
    for (const entry of CATALOG.filter((candidate) => seeds.includes(candidate.seed) && this.installed[candidate.member])) {
      const found = [...this.files.values()].find(({ hook }) => hook.seed === entry.seed);

      if (found) {
        this.put({ ...found.hook, action: { type: entry.type, text: entry.text }, updatedAt: new Date().toISOString().replace(/\.\d+Z$/, 'Z'), updatedBy: 'desktop' });
      } else {
        this.put(familyHook(entry));
      }
    }

    this.notify();

    return this.list(null);
  }

  /** A member was installed: its missing family hooks come back, none is overwritten. */
  memberInstalled(requirement: HookRequirement): void {
    this.installed[requirement] = true;
    this.applyFamily(CATALOG.filter((entry) => entry.member === requirement && this.entryStatus(entry) === 'missing').map((entry) => entry.seed));
  }

  /** An agent writes a new hook through the Harness Desktop MCP: the file is there, and nobody approved it. */
  agentAdds(name: string, description: string | null = null): HookView {
    const now = new Date().toISOString().replace(/\.\d+Z$/, 'Z');
    const hook: Hook = {
      id: hookId(),
      name,
      description,
      scope: 'global',
      project: null,
      event: 'turnEnd',
      matcher: null,
      action: { type: 'command', command: 'bun test', timeoutSecs: null },
      enabled: true,
      requires: [],
      seed: null,
      createdAt: now,
      createdBy: 'agent',
      updatedAt: now,
      updatedBy: 'agent',
    };

    this.files.set(hook.id, { hook, approved: null, changed: false });
    this.notify();

    return this.view(this.files.get(hook.id) as Stored);
  }

  /** An agent rewrites a hook again through the MCP: its bytes change, and what was approved stays the last version the user saw. */
  agentChanges(id: string): void {
    const stored = this.files.get(id);

    if (!stored) {
      throw IpcFailure.from({ code: 'notFound', message: `hook ${id} was not found` });
    }

    const now = new Date().toISOString().replace(/\.\d+Z$/, 'Z');

    stored.approved ??= null;
    stored.changed = stored.approved !== null;
    stored.hook = { ...stored.hook, name: `${stored.hook.name} v2`, updatedAt: now, updatedBy: 'agent' };
    this.notify();
  }

  listen(onEvent: () => void): null {
    this.listener = onEvent;

    return null;
  }

  /**
   * What the desktop puts in front of the first message of a chat in this project: the approved, enabled `sessionStart`
   * context hooks whose requirements are met, as the format composes them. Empty when there are none.
   */
  sessionContext(project: string | null): string {
    const elements = [...this.files.values()]
      .filter((stored) => this.statusOf(stored) === 'active')
      .map(({ hook }) => hook)
      .filter((hook) => hook.event === 'sessionStart' && hook.action.type === 'addContext' && (hook.scope === 'global' || hook.project === project));

    if (elements.length === 0) {
      return '';
    }

    const lines = elements.map((hook) => `<hook name="${escapeName(hook.name)}">${hook.action.type === 'command' ? '' : escapeText(hook.action.text)}</hook>`);

    return `<harness-hook-context>\n${lines.join('\n')}\n</harness-hook-context>\n\n`;
  }

  /**
   * What "Check again" sends a chat in this project: the active "Save what you learned" follow-up as the user keeps it (name and text), else
   * the catalog's version, as the format composes a follow-up.
   */
  memorySaveFollowUp(project: string | null): string {
    const configured = [...this.files.values()]
      .filter((stored) => this.statusOf(stored) === 'active')
      .map(({ hook }) => hook)
      .find((hook) => hook.seed === 'memory-save' && hook.action.type === 'followUp' && (hook.scope === 'global' || hook.project === project));
    const catalog = CATALOG.find((entry) => entry.seed === 'memory-save');

    const name = configured?.name ?? catalog?.name ?? '';
    const text = configured?.action.type === 'followUp' ? configured.action.text : (catalog?.text ?? '');

    return `<harness-hook-followup name="${escapeName(name)}">${escapeText(text)}</harness-hook-followup>`;
  }
}

function escapeName(name: string): string {
  return name.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
}

function escapeText(text: string): string {
  return text.replace(/<(?=\/?(?:hook|harness-hook))/gi, '&lt;');
}
