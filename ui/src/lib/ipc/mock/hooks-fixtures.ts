import type { Hook } from '../bindings/Hook';
import type { HookEvent } from '../bindings/HookEvent';
import type { HookFamilyEntry } from '../bindings/HookFamilyEntry';
import type { HookSeed } from '../bindings/HookSeed';
import { fixtures } from './fixtures';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

/** The project the sample project hooks belong to. */
export const HOOKS_PROJECT = fixtures.recentProjects[0].path;

function ago(ms: number): string {
  return new Date(Date.now() - ms).toISOString().replace(/\.\d+Z$/, 'Z');
}

/** A family hook of the catalog: the hook a member's tools come with. */
export interface CatalogEntry {
  seed: HookSeed;
  member: HookFamilyEntry['member'];
  name: string;
  event: HookEvent;
  type: 'addContext' | 'followUp';
  /** The text the catalog has now. */
  text: string;
}

export const CATALOG: readonly CatalogEntry[] = [
  {
    seed: 'memory-search',
    member: 'harness-memory',
    name: 'Search Harness Memory first',
    event: 'sessionStart',
    type: 'addContext',
    text: 'Before you start, search Harness Memory for notes about this task (note_search) and check the saved scripts (script_list). Reuse what fits.',
  },
  {
    seed: 'memory-save',
    member: 'harness-memory',
    name: 'Save what you learned',
    event: 'turnEnd',
    type: 'followUp',
    text: 'If this turn taught you something worth keeping, save it to Harness Memory (note_save or note_update). Save any command sequence you would run again as a script (script_save). Reply briefly, or say nothing needs saving.',
  },
  {
    seed: 'dashboard-limits',
    member: 'harness-dashboard',
    name: 'Check limits before long work',
    event: 'sessionStart',
    type: 'addContext',
    text: 'Before a long or expensive task, check the plan limits and budgets with Harness Dashboard (limits, budget_status). Say so if a limit is close.',
  },
  {
    seed: 'useful-skills',
    member: 'harness-memory',
    name: 'Check Useful Skills first',
    event: 'sessionStart',
    type: 'addContext',
    text: 'If Useful Skills are installed, read the ones that apply before you act: decide the workflow first (plan, implement, review), then read any other skill that fits the task and follow its limits (for example the 500-line file maximum). Do not skip a matching skill.',
  },
];

/** The text the memory-save hook had before the catalog changed: the sample of an outdated family hook. */
export const OLD_SAVE_TEXT = 'Before you finish, save what you learned about this task to Harness Memory (memory_save). Keep each note short and reusable.';

let counter = 0;

/** A 32 character id, like the real ones: stable per call order, so a preview looks the same on every load. */
export function hookId(): string {
  counter += 1;

  return counter.toString(16).padStart(32, '0');
}

/** A family hook as the desktop seeds it. */
export function familyHook(entry: CatalogEntry, text: string = entry.text): Hook {
  return {
    id: hookId(),
    name: entry.name,
    description: null,
    scope: 'global',
    project: null,
    event: entry.event,
    matcher: null,
    action: { type: entry.type, text },
    enabled: true,
    requires: [entry.member],
    seed: entry.seed,
    createdAt: ago(9 * 24 * HOUR),
    createdBy: 'desktop',
    updatedAt: ago(9 * 24 * HOUR),
    updatedBy: 'desktop',
  };
}

/** The sample hook an agent wrote, in the version the user approved. */
export function lintApproved(id: string): Hook {
  return {
    id,
    name: 'Run lint after edits',
    description: 'Catches style problems while the agent is still working.',
    scope: 'project',
    project: HOOKS_PROJECT,
    event: 'toolEnd',
    matcher: 'edit',
    action: { type: 'command', command: 'bun run lint', timeoutSecs: null },
    enabled: true,
    requires: [],
    seed: null,
    createdAt: ago(6 * 24 * HOUR),
    createdBy: 'agent',
    updatedAt: ago(5 * 24 * HOUR),
    updatedBy: 'desktop',
  };
}

/** The same hook as an agent changed it a few minutes ago: a wider filter, a quieter and longer command. It waits for approval. */
export function lintChanged(approved: Hook): Hook {
  return { ...approved, matcher: 'edit|write', action: { type: 'command', command: 'bun run lint --quiet', timeoutSecs: 20 }, updatedAt: ago(4 * MINUTE), updatedBy: 'agent' };
}

/** A hook of the user's own, for this project, that is switched off. */
export function testsReminder(): Hook {
  return {
    id: hookId(),
    name: 'Remind about tests',
    description: null,
    scope: 'project',
    project: HOOKS_PROJECT,
    event: 'promptSubmit',
    matcher: null,
    action: { type: 'addContext', text: 'Run the test suite before you say you are done, and add a test for any behaviour you change.' },
    enabled: false,
    requires: [],
    seed: null,
    createdAt: ago(2 * 24 * HOUR),
    createdBy: 'desktop',
    updatedAt: ago(24 * HOUR),
    updatedBy: 'desktop',
  };
}

export { fakeSha } from './fake-sha';
