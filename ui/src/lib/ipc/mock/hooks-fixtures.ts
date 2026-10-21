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
    text: 'Before you start, search Harness Memory for notes about this task (note_search) and check the saved scripts (script_list). Reuse what fits. Pass project_path on every Harness Memory call: the absolute path of the repository you are working in (its git root), so nothing is read from or saved to another repository.',
  },
  {
    seed: 'memory-save',
    member: 'harness-memory',
    name: 'Save what you learned',
    event: 'turnEnd',
    type: 'followUp',
    text: 'Most turns need no note: reply "nothing to save" unless this turn produced knowledge that a future agent or a teammate could not get from the code or git history, such as the root cause of a non-obvious bug and its fix, a decision with the reason behind it, a constraint or gotcha, or where something non-obvious lives. Never save routine work: installing or configuring tools or MCP servers, running commands, editing files, progress or status, or anything this turn merely did. Before saving, search with note_search and update the existing note (note_update, by id) instead of adding a near duplicate. Write a specific title and a body that stands alone for a reader with no context: what, why, and the repo-relative paths involved; no transcripts or step lists. Save at most one note per turn. Save a command sequence as a script (script_save) only when you have run it more than once and it will recur. Always pass project_path (the absolute path of the repository you are working in, its git root) to note_save, note_update and script_save: without it a note can land in the wrong repository. Reply briefly.',
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
  {
    seed: 'branch-sync',
    member: 'harness-memory',
    name: 'Check the branch against main',
    event: 'sessionStart',
    type: 'addContext',
    text: "At the start of the session, compare the branch you are working on with the repository's main branch: main, master or prod, whichever exists (git symbolic-ref refs/remotes/origin/HEAD names the default). Run git fetch, find which of the two has the most recent commits, and list the commits of the main branch your branch does not contain (git log HEAD..origin/<main> --oneline). If there are any, pull them into your branch unless the work tree is dirty or the merge would conflict, then tell me what came in and whether it touches the files in the scope of this work. Never discard local changes or rewrite history; ask me first when unsure.",
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
