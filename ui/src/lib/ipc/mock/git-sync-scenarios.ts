import type { BranchRow } from '../bindings/BranchRow';
import type { BranchSync } from '../bindings/BranchSync';
import type { ConflictFile } from '../bindings/ConflictFile';
import { fixtures } from './fixtures';

/**
 * What the branch popover finds in a preview opened with `?sync=...`:
 * - `ok` (default) the generated sample: 2 commits ahead, 1 behind, 4 changed files, main has nothing new
 * - `behind` main is 7 commits ahead of the branch, nothing uncommitted
 * - `dirty` the same with 3 uncommitted files, so a rebase stashes them
 * - `clash` the same, but a rebase or merge stops with conflicts
 * - `conflict` a rebase is stopped with conflicts, `merge` a merge is, `resolved` a rebase waits for its continue
 * - `onmain` the branch is main itself and the remote is 3 ahead
 * - `unpublished` the branch is not on the remote yet
 * - `nomain` no main branch was found, `noremote` the repository has no remote, `detached` HEAD is on a commit
 * - `offline` the remote cannot be reached, `stash` a stash of the user's changes is waiting
 * - `stashclash` the user's changes, put back from the stash, conflict with main (no rebase or merge is running)
 * - `stashapplied` the same conflicts are resolved: the changes are back and the stash holds only a copy
 */
export const SYNC_SCENARIOS = [
  'ok',
  'behind',
  'dirty',
  'clash',
  'conflict',
  'merge',
  'resolved',
  'onmain',
  'unpublished',
  'nomain',
  'noremote',
  'detached',
  'offline',
  'stash',
  'stashclash',
  'stashapplied',
] as const;

export type SyncScenario = (typeof SYNC_SCENARIOS)[number];

export function syncScenarioFromLocation(search: string = globalThis.location?.search ?? ''): SyncScenario {
  const name = new URLSearchParams(search).get('sync');

  return SYNC_SCENARIOS.find((scenario) => scenario === name) ?? 'ok';
}

/** `?branches=1500` fills the repository with that many more branches, to see the list hold up. */
export function extraBranchesFromLocation(search: string = globalThis.location?.search ?? ''): number {
  const count = Number(new URLSearchParams(search).get('branches'));

  return Number.isInteger(count) && count > 0 ? Math.min(count, 20_000) : 0;
}

/** Changes the project folder holds: conflicts in it are clickable, the one outside it is not. */
const CONFLICTS: ConflictFile[] = [
  { path: 'ui/src/features/chat/ComposerFooter.svelte', inProject: true },
  { path: 'ui/src/lib/stores/git.svelte.ts', inProject: true },
  { path: 'src-tauri/src/commands/git.rs', inProject: false },
];

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;

function ago(milliseconds: number): string {
  return new Date(Date.now() - milliseconds).toISOString();
}

/** The branch state a preview starts with: the generated sample, bent to the scenario. */
export function startingState(scenario: SyncScenario): BranchSync {
  const base: BranchSync = {
    ...structuredClone(fixtures.git.sync),
    lastFetched: ago(3 * MINUTE_MS),
  };
  const main = base.main;
  const moved = {
    ...base,
    ahead: 2,
    behind: 0,
    trackedChanges: 0,
    untracked: 0,
    main: main && { ...main, source: 'remote' as const, newCommits: 7 },
  };

  switch (scenario) {
    case 'behind':
      return moved;
    case 'dirty':
    case 'clash':
      return { ...moved, trackedChanges: 3, untracked: 1 };
    case 'conflict':
    case 'resolved':
      return {
        ...moved,
        operation: {
          kind: 'rebase',
          onto: 'main',
          conflicts: scenario === 'conflict' ? CONFLICTS : [],
        },
        unmerged: scenario === 'conflict' ? CONFLICTS : [],
        trackedChanges: 2,
      };
    case 'merge':
      return {
        ...moved,
        operation: {
          kind: 'merge',
          onto: 'main',
          conflicts: CONFLICTS.slice(0, 2),
        },
        unmerged: CONFLICTS.slice(0, 2),
      };
    case 'onmain':
      return {
        ...base,
        branch: 'main',
        upstream: 'origin/main',
        ahead: 0,
        behind: 3,
        trackedChanges: 0,
        untracked: 0,
        main: main && { ...main, newCommits: 0, isCurrent: true },
      };
    case 'unpublished':
      return { ...base, upstream: null, ahead: 2, behind: 0 };
    case 'nomain':
      return { ...base, main: null };
    case 'noremote':
      return {
        ...base,
        remote: null,
        upstream: null,
        ahead: 0,
        behind: 0,
        lastFetched: null,
      };
    case 'detached':
      return {
        ...base,
        branch: null,
        upstream: null,
        ahead: 0,
        behind: 0,
        trackedChanges: 0,
        untracked: 0,
      };
    case 'stashclash':
      return {
        ...moved,
        trackedChanges: 0,
        untracked: 0,
        unmerged: CONFLICTS,
        stashKept: true,
        stashApplied: true,
      };
    case 'stashapplied':
      return {
        ...base,
        trackedChanges: 0,
        untracked: 0,
        stashKept: true,
        stashApplied: true,
      };
    case 'stash':
      return { ...base, trackedChanges: 0, untracked: 0, stashKept: true };
    default:
      return base;
  }
}

/** Local branches of the sample repository, newest first, then the ones only the remote has. */
export function startingBranches(current: string | null, extra: number): BranchRow[] {
  const local: [string, number][] = [
    ['feat/empty-input', 5 * MINUTE_MS],
    ['main', 2 * HOUR_MS],
    ['fix/hooks-first-message', 26 * HOUR_MS],
    ['feat/memory-search-v2', 3 * 24 * HOUR_MS],
    ['chore/release-0.40', 6 * 24 * HOUR_MS],
  ];
  const remote: [string, number][] = [
    ['feat/opencode-sessions', 8 * HOUR_MS],
    ['fix/windows-paths', 4 * 24 * HOUR_MS],
  ];
  const rows: BranchRow[] = [
    ...local.map(([name, age]) => ({
      name,
      isCurrent: name === current,
      remoteOnly: false,
      lastCommitDate: ago(age),
    })),
    ...remote.map(([name, age]) => ({
      name,
      isCurrent: false,
      remoteOnly: true,
      lastCommitDate: ago(age),
    })),
  ];

  for (let index = 1; index <= extra; index += 1) {
    const name = `topic/${String(index).padStart(4, '0')}-${['parser', 'layout', 'cache', 'rail', 'diff'][index % 5]}`;
    rows.push({
      name,
      isCurrent: false,
      remoteOnly: index % 3 === 0,
      lastCommitDate: ago((7 + index) * 24 * HOUR_MS),
    });
  }

  return rows;
}
