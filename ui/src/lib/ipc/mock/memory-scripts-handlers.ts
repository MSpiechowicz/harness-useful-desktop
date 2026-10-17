import type { MemoryDeleteAll } from '../bindings/MemoryDeleteAll';
import type { MemoryScriptsAvailability } from '../bindings/MemoryScriptsAvailability';
import type { MemoryScriptsPage } from '../bindings/MemoryScriptsPage';
import type { ScriptDetail } from '../bindings/ScriptDetail';
import type { ScriptSort } from '../bindings/ScriptSort';
import type { ScriptSummary } from '../bindings/ScriptSummary';
import type { CommandArgs } from '../commands';
import { IpcFailure } from '../transport';
import { deletedAll, requireServer } from './memory-delete-all';
import { sampleScripts, summaryOf } from './memory-scripts-fixtures';
import { sampleRunRequests } from './script-request-fixtures';

/** Browser-preview switch `?scripts=`: what the Memory view finds. Anything else shows the sample scripts. `outdated` has a newer Harness Memory release to update to, `outdatedNoRelease` is a source build (`1.0.1-dev`) with none. `request` also has two agents ask to run a script a moment after the app starts listening. */
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
  repo: (script: ScriptSummary) => script.scope === 'repo',
  global: (script: ScriptSummary) => script.scope === 'global',
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

/** One page of scripts as the server answers: filtered by the query (name or description) and the group, counted per group, sorted and cut. */
export function pageOfScripts(visible: ScriptSummary[], { offset, limit = DEFAULT_LIMIT, query = '', group, sort = 'updated', order }: CommandArgs<'memory_scripts_list'>): Omit<MemoryScriptsPage, 'availability'> {
  const needle = query.trim().toLowerCase();
  const answering = visible.filter((script) => needle === '' || script.name.toLowerCase().includes(needle) || script.description.toLowerCase().includes(needle));
  const direction = (order ?? (sort === 'updated' ? 'desc' : 'asc')) === 'desc' ? -1 : 1;
  const compare = SCRIPT_ORDERS[sort];
  const chosen = answering.filter((script) => group === undefined || GROUPS[group](script));

  return {
    total: chosen.length,
    groups: {
      needsReview: answering.filter(GROUPS.needsReview).length,
      repo: answering.filter(GROUPS.repo).length,
      global: answering.filter(GROUPS.global).length,
    },
    scripts: [...chosen].sort((a, b) => neverRunLast(sort, a, b) || direction * (compare(a, b) || a.id - b.id)).slice(offset, offset + limit),
  };
}

/** The scripts of Harness Memory kept in memory: `memory_scripts_list/script_get/script_delete/scripts_delete_all` like the real backend, without a server. */
export interface ScriptHandlers {
  memory_scripts_list: (args: CommandArgs<'memory_scripts_list'>) => MemoryScriptsPage;
  memory_script_get: (args: CommandArgs<'memory_script_get'>) => ScriptDetail;
  memory_script_delete: (args: CommandArgs<'memory_script_delete'>) => null;
  memory_scripts_delete_all: (args: CommandArgs<'memory_scripts_delete_all'>) => MemoryDeleteAll;
  memory_script_requests_listen: (args: CommandArgs<'memory_script_requests_listen'>) => null;
  memory_script_decide: (args: CommandArgs<'memory_script_decide'>) => null;
  memory_scripts_trust: (args: CommandArgs<'memory_scripts_trust'>) => null;
}

/** How long the Memory server waits for an answer. */
const APPROVAL_WINDOW_MS = 45_000;
/** The requests appear this long after the app starts listening, so the preview shows the prompt arriving over a view. */
const REQUEST_DELAY_MS = import.meta.env.MODE === 'test' ? 0 : 1200;

export function memoryScriptsHandlers(scenario: () => ScriptScenario = scriptScenarioFromLocation): ScriptHandlers {
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
        return { availability, total: 0, groups: { needsReview: 0, repo: 0, global: 0 }, scripts: [] };
      }

      return { availability, ...pageOfScripts(visible(args.project).map(summaryOf), args) };
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
