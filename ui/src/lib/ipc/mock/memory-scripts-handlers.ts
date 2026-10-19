import type { MemoryDeleteAll } from '../bindings/MemoryDeleteAll';
import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
import type { MemoryScriptsPage } from '../bindings/MemoryScriptsPage';
import type { ScriptDetail } from '../bindings/ScriptDetail';
import type { ScriptSaved } from '../bindings/ScriptSaved';
import type { ScriptSort } from '../bindings/ScriptSort';
import type { ScriptSummary } from '../bindings/ScriptSummary';
import type { CommandArgs } from '../commands';
import { IpcFailure } from '../transport';
import { fakeSha } from './fake-sha';
import { deletedAll, requireServer } from './memory-delete-all';
import { combinesFilters } from './memory-notes-handlers';
import { sampleScripts, summaryOf } from './memory-scripts-fixtures';
import { sampleRunRequests } from './script-request-fixtures';

/** Browser-preview switch `?scripts=`: what the Scripts view finds. Anything else shows the sample scripts. `outdated` has a newer Harness Memory release to update to, `outdatedNoRelease` is a source build (`1.0.1-dev`) with none. `request` also has two agents ask to run a script a moment after the app starts listening. */
export const SCRIPT_SCENARIOS = ['ok', 'request', 'empty', 'notInstalled', 'outdated', 'outdatedNoRelease', 'failed'] as const;
export type ScriptScenario = (typeof SCRIPT_SCENARIOS)[number];

export function scriptScenarioFromLocation(search: string = globalThis.location?.search ?? ''): ScriptScenario {
  const name = new URLSearchParams(search).get('scripts');

  return SCRIPT_SCENARIOS.find((scenario) => scenario === name) ?? 'ok';
}

/** Whether the mock server keeps scripts at all: the settings row follows it, like the real one follows the installed tools. */
export function scriptsSupported(scenario: ScriptScenario = scriptScenarioFromLocation()): boolean {
  return scenario !== 'notInstalled' && scenario !== 'outdated' && scenario !== 'outdatedNoRelease';
}

/** The scenario once Harness Memory was installed or updated: a missing or old one is fine now, the others stay as they are. */
export function scenarioAfterInstall(scenario: ScriptScenario): ScriptScenario {
  return scenario === 'notInstalled' || scenario === 'outdated' ? 'ok' : scenario;
}

/** The code as the backend compares it: line endings as LF, no trailing spaces or tabs on a line, no trailing blank lines. */
function normalisedCode(body: string): string {
  // Split on LF only, as the backend does: a multiline `$` would also end a line at U+2028 and U+2029.
  return body
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map((line) => line.replace(/[ \t]+$/, ''))
    .join('\n')
    .replace(/\n+$/, '');
}

export function availabilityOf(scenario: ScriptScenario): MemoryScriptsAvailability {
  switch (scenario) {
    case 'notInstalled':
      return { kind: 'notInstalled' };
    case 'outdated':
    case 'outdatedNoRelease':
      return { kind: 'outdated' };
    case 'failed':
      return { kind: 'failed', message: 'harness-memory mcp exited before it answered (exit code 1)' };
    case 'ok':
    case 'request':
    case 'empty':
      return { kind: 'available' };
  }
}

const DEFAULT_LIMIT = 50;

const GROUPS = {
  needsReview: (script: ScriptSummary) => script.approval === 'needed',
  repo: (script: ScriptSummary) => script.scope === 'repo' && script.approval !== 'needed',
  global: (script: ScriptSummary) => script.scope === 'global' && script.approval !== 'needed',
} as const;

const SCOPES = {
  repo: (script: ScriptSummary) => script.scope === 'repo',
  global: (script: ScriptSummary) => script.scope === 'global',
} as const;

const REVIEWS = {
  needed: (script: ScriptSummary) => script.approval === 'needed',
  trusted: (script: ScriptSummary) => script.approval !== 'needed',
} as const;

type ScriptOrder = (a: ScriptSummary, b: ScriptSummary) => number;

/** How each sort orders two scripts, ascending (`scope`: global first); ties go by id. `lastRun` leaves the never run ones to {@link neverRunLast}. */
const SCRIPT_ORDERS: Record<ScriptSort, ScriptOrder> = {
  updated: (a, b) => Date.parse(a.updatedAt) - Date.parse(b.updatedAt),
  name: (a, b) => a.name.localeCompare(b.name),
  language: (a, b) => a.language.localeCompare(b.language),
  scope: (a, b) => a.scope.localeCompare(b.scope),
  place: (a, b) => a.place.localeCompare(b.place),
  runs: (a, b) => a.runCount - b.runCount,
  lastRun: (a, b) => Date.parse(a.lastRunAt ?? '') - Date.parse(b.lastRunAt ?? '') || 0,
};

/** A script never run comes after the others by `lastRun`, whichever the order. */
function neverRunLast(sort: ScriptSort, a: ScriptSummary, b: ScriptSummary): number {
  return sort === 'lastRun' ? Number(a.lastRunAt === null) - Number(b.lastRunAt === null) : 0;
}

/**
 * One page of scripts as the server answers: filtered by the query (name or description), the scope, the review and the group, sorted and cut.
 * The groups (disjoint: a script that needs review is in no scope group) are counted under the query, the scopes under the query and the review, the review under the query and the scope. A group asked for
 * together with a scope or review is refused. `combined: false` is an older server: it drops the scope and the review and counts neither.
 */
export function pageOfScripts(
  visible: ScriptSummary[],
  { offset, limit = DEFAULT_LIMIT, query = '', scope, review, group, sort = 'updated', order }: CommandArgs<'memory_scripts_list'>,
  combined = true,
): Omit<MemoryScriptsPage, 'availability'> {
  if (combined && group !== undefined && (scope !== undefined || review !== undefined)) {
    throw IpcFailure.from({ code: 'invalidInput', message: 'group cannot be combined with scope or review' });
  }

  const needle = query.trim().toLowerCase();
  const answering = visible.filter((script) => needle === '' || script.name.toLowerCase().includes(needle) || script.description.toLowerCase().includes(needle));
  const direction = (order ?? (sort === 'updated' ? 'desc' : 'asc')) === 'desc' ? -1 : 1;
  const compare = SCRIPT_ORDERS[sort];
  const inScope = answering.filter((script) => !combined || scope === undefined || script.scope === scope);
  const inReview = answering.filter((script) => !combined || review === undefined || REVIEWS[review](script));
  const chosen = answering.filter((script) => inScope.includes(script) && inReview.includes(script) && (group === undefined || GROUPS[group](script)));

  return {
    total: chosen.length,
    groups: {
      needsReview: answering.filter(GROUPS.needsReview).length,
      repo: answering.filter(GROUPS.repo).length,
      global: answering.filter(GROUPS.global).length,
    },
    scopes: combined ? { repo: inReview.filter(SCOPES.repo).length, global: inReview.filter(SCOPES.global).length } : null,
    review: combined ? { needed: inScope.filter(REVIEWS.needed).length, trusted: inScope.filter(REVIEWS.trusted).length } : null,
    combinedFilters: combined,
    scripts: [...chosen].sort((a, b) => neverRunLast(sort, a, b) || direction * (compare(a, b) || a.id - b.id)).slice(offset, offset + limit),
  };
}

/** The scripts of Harness Memory kept in memory: `memory_scripts_list/script_get/script_delete/scripts_delete_all/script_save` like the real backend, without a server. */
export interface ScriptHandlers {
  memory_scripts_list: (args: CommandArgs<'memory_scripts_list'>) => MemoryScriptsPage;
  memory_script_get: (args: CommandArgs<'memory_script_get'>) => ScriptDetail;
  memory_script_delete: (args: CommandArgs<'memory_script_delete'>) => null;
  memory_scripts_delete_all: (args: CommandArgs<'memory_scripts_delete_all'>) => MemoryDeleteAll;
  memory_script_requests_listen: (args: CommandArgs<'memory_script_requests_listen'>) => null;
  memory_script_decide: (args: CommandArgs<'memory_script_decide'>) => null;
  memory_scripts_trust: (args: CommandArgs<'memory_scripts_trust'>) => null;
  memory_script_save: (args: CommandArgs<'memory_script_save'>) => ScriptSaved;
}

/** The repository key of new repository scripts when no script has one yet. */
const MOCK_REPO = 'github.com/useful/harness-desktop';

/** How long the Memory server waits for an answer. */
const APPROVAL_WINDOW_MS = 45_000;
/** The requests appear this long after the app starts listening, so the preview shows the prompt arriving over a view. */
const REQUEST_DELAY_MS = import.meta.env.MODE === 'test' ? 0 : 1200;

export function memoryScriptsHandlers(scenario: () => ScriptScenario = scriptScenarioFromLocation, combined: () => boolean = combinesFilters): ScriptHandlers {
  let scripts: ScriptDetail[] | null = null;
  const current = (): ScriptDetail[] => (scripts ??= scenario() === 'ok' || scenario() === 'request' ? sampleScripts() : []);

  /** A project sees its repository's scripts and the global ones, no project only the global ones. */
  const visible = (project: string | null): ScriptDetail[] => current().filter((script) => script.scope === 'global' || project !== null);

  const find = (id: number, project: string | null): ScriptDetail => {
    const script = visible(project).find((candidate) => candidate.id === id);
    if (!script) {
      throw IpcFailure.from({ code: 'notFound', message: `script ${id} was not found` });
    }

    return script;
  };

  /** Requests an agent waits on. An answer, or the end of the wait, takes one out. */
  const pending = new Set<string>();

  const approve = (script: ScriptDetail): void => {
    script.trusted = true;
    script.approval = 'approved';
  };

  return {
    // The `request` scenario has two agents ask a moment after the app starts listening, and give up after the server's window.
    memory_script_requests_listen: ({ onRequest }) => {
      if (scenario() !== 'request') {
        return null;
      }

      setTimeout(() => {
        for (const request of sampleRunRequests(current())) {
          pending.add(request.requestId);
          onRequest({ type: 'request', request });

          setTimeout(() => {
            if (pending.delete(request.requestId)) {
              onRequest({ type: 'gone', requestId: request.requestId });
            }
          }, APPROVAL_WINDOW_MS);
        }
      }, REQUEST_DELAY_MS);

      return null;
    },
    memory_scripts_list: (args) => {
      const availability = availabilityOf(scenario());
      if (availability.kind !== 'available') {
        return { availability, total: 0, groups: { needsReview: 0, repo: 0, global: 0 }, scopes: null, review: null, combinedFilters: combined(), scripts: [] };
      }

      return { availability, ...pageOfScripts(visible(args.project).map(summaryOf), args, combined()) };
    },
    memory_script_get: ({ id, project }) => find(id, project),
    memory_script_delete: ({ id, project }) => {
      const script = find(id, project);
      scripts = current().filter((candidate) => candidate !== script);

      return null;
    },
    memory_scripts_delete_all: ({ project = null }) => {
      requireServer(availabilityOf(scenario()));

      const gone = visible(project);
      scripts = current().filter((candidate) => !gone.includes(candidate));

      return deletedAll(gone);
    },
    memory_script_decide: ({ requestId, allow }) => {
      if (!pending.delete(requestId)) {
        throw IpcFailure.from({ code: 'notFound', message: 'the agent stopped waiting for this script' });
      }

      const asked = sampleRunRequests(current()).find((request) => request.requestId === requestId);
      const script = current().find((candidate) => candidate.id === asked?.script.id);

      if (allow && script) {
        approve(script);
      }

      return null;
    },
    // Like the real backend: an edit (`base`) of a script deleted or changed since it opened is a `conflict`, nothing saved. A name taken in
    // that scope is `exists` unless replacing, and a script saved here is trusted. A script waiting for approval stays so after an edit that
    // left its code as it opened (a description, timeout, language, line-ending or trailing-whitespace edit alone) or a replace with its code.
    memory_script_save: ({ project, draft, replace, base }) => {
      requireServer(availabilityOf(scenario()));

      if (draft.scope === 'repo' && project === null) {
        throw IpcFailure.from({ code: 'invalidInput', message: 'a repository script needs a project' });
      }

      const name = draft.name.trim();
      const existing = visible(project).find((candidate) => candidate.name === name && candidate.scope === draft.scope);
      if (base !== undefined && existing?.sha256 !== base) {
        throw IpcFailure.from({ code: 'conflict', message: 'the script changed since you opened it' });
      }

      if (existing && !replace) {
        return { outcome: 'exists', id: existing.id, script: null };
      }

      const now = new Date().toISOString();
      const waiting = existing?.approval === 'needed';
      const agentsCode = existing?.approval === 'needed' && normalisedCode(existing.body) === normalisedCode(draft.body);
      const saved: ScriptDetail = {
        id: existing?.id ?? Math.max(0, ...current().map((candidate) => candidate.id)) + 1,
        name,
        description: draft.description.trim(),
        language: draft.language,
        scope: draft.scope,
        repo: draft.scope === 'repo' ? (current().find((candidate) => candidate.repo !== null)?.repo ?? MOCK_REPO) : null,
        sha256: fakeSha([draft.language, draft.body]),
        timeoutSecs: draft.timeoutSecs,
        place: 'local',
        author: { source: 'harness-desktop', root: draft.scope === 'repo' ? project : null },
        createdAt: existing?.createdAt ?? now,
        updatedAt: now,
        runCount: existing?.runCount ?? 0,
        lastRunAt: existing?.lastRunAt ?? null,
        lastExitCode: existing?.lastExitCode ?? null,
        trusted: !agentsCode,
        approval: agentsCode ? 'needed' : waiting ? 'approved' : 'local',
        body: draft.body,
      };

      scripts = existing ? current().map((candidate) => (candidate === existing ? saved : candidate)) : [...current(), saved];

      return { outcome: existing ? 'replaced' : 'created', id: saved.id, script: summaryOf(saved) };
    },
    // Like the real server: a script that changed since it was reviewed is refused, and the others are approved anyway.
    memory_scripts_trust: ({ items, project }) => {
      const found = items.map((item) => ({ script: find(item.id, project), sha256: item.sha256 }));
      found.filter(({ script, sha256 }) => script.sha256 === sha256).forEach(({ script }) => approve(script));

      if (found.some(({ script, sha256 }) => script.sha256 !== sha256)) {
        throw IpcFailure.from({ code: 'conflict', message: 'a script changed since it was reviewed' });
      }

      return null;
    },
  };
}
