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
import { deletion, requireServer } from './memory-delete-all';
import { type Reach, reaches } from './memory-reach';
import { combinesFilters } from './memory-notes-handlers';
import { type CopyStores, type TeamScriptsHandlers, teamScriptsHandlers } from './memory-notes-team-handlers';
import { sampleScripts, summaryOf } from './memory-scripts-fixtures';
import { sampleRunRequests } from './script-request-fixtures';

/** Browser-preview switch `?scripts=`: what the Scripts view finds. Anything else shows the sample scripts. `outdated` has a newer Harness Memory release to update to, `outdatedNoRelease` is a source build (`1.0.1-dev`) with none. `request` also has two agents ask to run a script a moment after the app starts listening, `readonly` makes the remote scripts read-only (a scope that saves on this computer). */
export const SCRIPT_SCENARIOS = ['ok', 'request', 'readonly', 'empty', 'notInstalled', 'outdated', 'outdatedNoRelease', 'failed'] as const;
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
    case 'readonly':
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
 * One page of scripts as the server answers: filtered by the query (name or description), the scope, the review, the place and the group, sorted and cut.
 * The groups (disjoint: a script that needs review is in no scope group) are counted under the query, the scopes under the query, the review and the place,
 * the review under the query, the scope and the place, the places under the query, the scope and the review. A group asked for
 * together with a scope or review is refused. `combined: false` is an older server: it drops the scope, the review and the place and counts none of them.
 * A hidden script is listed only when `hidden` asks for them. `everything` is every script of the store, which the count of scripts on this computer takes in.
 */
export function pageOfScripts(
  visible: ScriptSummary[],
  { offset, limit = DEFAULT_LIMIT, query = '', scope, review, place, group, sort = 'updated', order, hidden }: CommandArgs<'memory_scripts_list'>,
  combined = true,
  everything: ScriptSummary[] = visible,
): Omit<MemoryScriptsPage, 'availability'> {
  if (combined && group !== undefined && (scope !== undefined || review !== undefined)) {
    throw IpcFailure.from({ code: 'invalidInput', message: 'group cannot be combined with scope or review' });
  }

  const needle = query.trim().toLowerCase();
  const answering = visible.filter((script) => script.hidden === (hidden ?? false) && (needle === '' || script.name.toLowerCase().includes(needle) || script.description.toLowerCase().includes(needle)));
  const onThisComputer = everything.filter((script) => script.place === 'local');
  const direction = (order ?? (sort === 'updated' ? 'desc' : 'asc')) === 'desc' ? -1 : 1;
  const compare = SCRIPT_ORDERS[sort];
  const inScope = answering.filter((script) => !combined || scope === undefined || script.scope === scope);
  const inReview = answering.filter((script) => !combined || review === undefined || REVIEWS[review](script));
  const inPlace = answering.filter((script) => !combined || place === undefined || script.place === place);
  const chosen = answering.filter((script) => inScope.includes(script) && inReview.includes(script) && inPlace.includes(script) && (group === undefined || GROUPS[group](script)));
  const reviewedHere = inReview.filter((script) => inPlace.includes(script));
  const inScopeHere = inScope.filter((script) => inPlace.includes(script));
  const reviewedInScope = inReview.filter((script) => inScope.includes(script));

  return {
    total: chosen.length,
    groups: {
      needsReview: answering.filter(GROUPS.needsReview).length,
      repo: answering.filter(GROUPS.repo).length,
      global: answering.filter(GROUPS.global).length,
    },
    scopes: combined ? { repo: reviewedHere.filter(SCOPES.repo).length, global: reviewedHere.filter(SCOPES.global).length } : null,
    review: combined ? { needed: inScopeHere.filter(REVIEWS.needed).length, trusted: inScopeHere.filter(REVIEWS.trusted).length } : null,
    places: combined ? { local: reviewedInScope.filter((script) => script.place === 'local').length, team: reviewedInScope.filter((script) => script.place === 'team').length } : null,
    combinedFilters: combined,
    scripts: [...chosen].sort((a, b) => neverRunLast(sort, a, b) || direction * (compare(a, b) || a.id - b.id)).slice(offset, offset + limit),
    hiddenCount: visible.filter((script) => script.hidden).length,
    localCounts: { repo: onThisComputer.filter(SCOPES.repo).length, global: onThisComputer.filter(SCOPES.global).length },
    offline: false,
    cachedAt: null,
  };
}

/** The scripts of Harness Memory kept in memory: `memory_scripts_list/script_get/script_delete/scripts_delete_all/script_save` like the real backend, without a server. */
export interface ScriptHandlers extends TeamScriptsHandlers {
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

/** `copyStores` is shared with the notes handlers, which serve the copy commands of both kinds. */
/** The sample scripts of a scenario; in `readonly` the remote ones cannot be changed from this computer. */
function withReadOnly(scenario: ScriptScenario): ScriptDetail[] {
  if (scenario !== 'ok' && scenario !== 'request' && scenario !== 'readonly') {
    return [];
  }

  return sampleScripts().map((script) => ({ ...script, readOnly: scenario === 'readonly' && script.place === 'team' }));
}

export function memoryScriptsHandlers(scenario: () => ScriptScenario = scriptScenarioFromLocation, combined: () => boolean = combinesFilters, copyStores: CopyStores = {}): ScriptHandlers {
  let scripts: ScriptDetail[] | null = null;
  const current = (): ScriptDetail[] => (scripts ??= withReadOnly(scenario()));

  /** A project sees its repository's scripts and the global ones, no project only the global ones; `repos` reads every repository or one. */
  const visible = (reach: Reach): ScriptDetail[] => current().filter((script) => reaches(script, reach));

  const find = (id: number, reach: Reach, hidden = false): ScriptDetail => {
    const script = visible(reach).find((candidate) => candidate.id === id && candidate.hidden === hidden);
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
    ...teamScriptsHandlers({ read: current, write: (next) => (scripts = next), availability: () => availabilityOf(scenario()) }, copyStores),
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
        return { availability, total: 0, groups: { needsReview: 0, repo: 0, global: 0 }, scopes: null, review: null, places: null, combinedFilters: combined(), scripts: [], hiddenCount: null, localCounts: null, offline: false, cachedAt: null };
      }

      return { availability, ...pageOfScripts(visible(args).map(summaryOf), args, combined(), current().map(summaryOf)) };
    },
    memory_script_get: ({ id, hidden, ...reach }) => find(id, reach, hidden),
    memory_script_delete: ({ id, ...reach }) => {
      const script = find(id, reach);
      scripts = current().filter((candidate) => candidate !== script);

      return null;
    },
    memory_scripts_delete_all: ({ project = null, repos, scope }) => {
      requireServer(availabilityOf(scenario()));

      // A remote script that is read-only here stays.
      const { gone, answer } = deletion(visible({ project, repos }), repos, scope);
      scripts = current().filter((candidate) => !gone.includes(candidate));

      return answer;
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
      const existing = visible({ project }).find((candidate) => candidate.name === name && candidate.scope === draft.scope && !candidate.readOnly);
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
        readOnly: false,
        hidden: false,
        body: draft.body,
      };

      scripts = existing ? current().map((candidate) => (candidate === existing ? saved : candidate)) : [...current(), saved];

      return { outcome: existing ? 'replaced' : 'created', id: saved.id, script: summaryOf(saved) };
    },
    // Like the real server: a script that changed since it was reviewed is refused, and the others are approved anyway.
    memory_scripts_trust: ({ items, ...reach }) => {
      const found = items.map((item) => ({ script: find(item.id, reach), sha256: item.sha256 }));
      found.filter(({ script, sha256 }) => script.sha256 === sha256).forEach(({ script }) => approve(script));

      if (found.some(({ script, sha256 }) => script.sha256 !== sha256)) {
        throw IpcFailure.from({ code: 'conflict', message: 'a script changed since it was reviewed' });
      }

      return null;
    },
  };
}
