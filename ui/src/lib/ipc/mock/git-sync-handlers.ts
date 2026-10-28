import type { BranchPage } from '../bindings/BranchPage';
import type { BranchRow } from '../bindings/BranchRow';
import type { BranchSync } from '../bindings/BranchSync';
import type { GitActionResult } from '../bindings/GitActionResult';
import type { SyncAction } from '../bindings/SyncAction';
import { isBranchName } from '../../git/branch-name';
import { IpcFailure } from '../transport';
import { extraBranchesFromLocation, startingBranches, startingState, syncScenarioFromLocation, type SyncScenario } from './git-sync-scenarios';
import type { MockState } from './state';
import type { Handlers } from './types';

type SyncCommands = 'git_sync_state' | 'git_fetch' | 'git_sync_action' | 'git_switch' | 'git_branch_search' | 'git_set_main_branch';

const MAX_SEARCH = 100;
const NOT_A_REPO: BranchSync = {
  isRepo: false,
  branch: null,
  head: null,
  upstream: null,
  remote: null,
  ahead: 0,
  behind: 0,
  main: null,
  operation: null,
  unmerged: [],
  trackedChanges: 0,
  untracked: 0,
  lastFetched: null,
  stashKept: false,
  stashApplied: false,
};

/** What the mock knows about one project's repository: its branch state, its branches and how far main is ahead. */
interface Repo {
  sync: BranchSync;
  branches: BranchRow[];
  /** Commits main has that a branch started anywhere else has not. */
  mainAhead: number;
}

function invalid(message: string): IpcFailure {
  return IpcFailure.from({ code: 'invalidInput', message });
}

function isRepo(state: MockState, project: string): boolean {
  const known = state.active?.path === project ? state.active : state.recents.find((recent) => recent.path === project);

  return known?.isGitRepo ?? false;
}

/**
 * In-memory branch sync: the generated sample state, bent by `?sync=` (see `SYNC_SCENARIOS`) and `?branches=1500`.
 * Actions change it the way git would, so a preview can be clicked through. A switch to `main` with uncommitted
 * files is refused, the way git refuses when the files overlap.
 */
export function gitSyncHandlers(state: MockState, scenario: () => SyncScenario = syncScenarioFromLocation): Pick<Handlers, SyncCommands> {
  const repos = new Map<string, Repo>();

  function repoOf(project: string): Repo {
    state.requireKnown(project);

    let repo = repos.get(project);
    if (!repo) {
      const sync = startingState(scenario());
      const picked = state.mainBranches.find((choice) => choice.project === project);
      repo = {
        sync,
        branches: startingBranches(sync.branch, extraBranchesFromLocation()),
        mainAhead: sync.main?.newCommits ?? 0,
      };
      repos.set(project, repo);

      if (picked && sync.main) {
        repo.sync.main = {
          ...sync.main,
          name: picked.branch,
          reference: picked.branch,
          source: 'picked',
          isCurrent: sync.branch === picked.branch,
        };
      }
    }

    return repo;
  }

  const result = (repo: Repo, outcome: GitActionResult['outcome']): GitActionResult => ({ outcome, state: structuredClone(repo.sync) });

  /** Stopped halfway, as a rebase or merge that hit conflicts. */
  function stop(repo: Repo, kind: 'rebase' | 'merge'): GitActionResult {
    const conflicts = startingState('conflict').operation?.conflicts ?? [];
    repo.sync.operation = {
      kind,
      onto: repo.sync.main?.name ?? null,
      conflicts,
    };
    repo.sync.unmerged = structuredClone(conflicts);

    return result(repo, 'stopped');
  }

  function catchUp(repo: Repo, kind: 'rebase' | 'merge'): GitActionResult {
    const main = repo.sync.main;
    if (!main || main.newCommits === 0) {
      return result(repo, 'upToDate');
    }

    if (scenario() === 'clash') {
      return stop(repo, kind);
    }

    repo.mainAhead = 0;
    repo.sync.main = { ...main, newCommits: 0 };
    repo.sync.trackedChanges = 0;
    repo.sync.ahead += kind === 'merge' ? 1 : 0;

    return result(repo, 'done');
  }

  function act(repo: Repo, action: SyncAction): GitActionResult {
    const sync = repo.sync;

    switch (action) {
      case 'push':
        sync.ahead = 0;

        return result(repo, 'done');
      case 'publish':
        sync.upstream = `${sync.remote}/${sync.branch}`;
        sync.ahead = 0;

        return result(repo, 'done');
      case 'pull':
        if (sync.behind === 0) {
          return result(repo, 'upToDate');
        }
        sync.behind = 0;

        return result(repo, 'done');
      case 'rebase':
      case 'merge':
        return catchUp(repo, action);
      case 'abort':
        if (!sync.operation) {
          throw invalid('no rebase or merge to abort');
        }
        sync.operation = null;
        sync.unmerged = [];

        return result(repo, 'done');
      case 'continue':
        if (!sync.operation || sync.operation.conflicts.length > 0) {
          throw invalid('fix the conflicts first');
        }
        sync.operation = null;
        sync.main = sync.main && { ...sync.main, newCommits: 0 };

        return result(repo, 'done');
      case 'applyStash':
        if (!sync.stashKept) {
          throw invalid('there is no stash of yours to apply');
        }
        if (sync.stashApplied) {
          throw invalid('the stash was applied already, drop its copy instead');
        }
        if (sync.unmerged.length > 0) {
          throw invalid('fix the conflicts first');
        }
        sync.stashKept = false;

        return result(repo, 'done');
      case 'dropStash':
        if (!sync.stashKept || !sync.stashApplied) {
          throw invalid('there is no applied copy of your stash to drop');
        }
        if (sync.unmerged.length > 0) {
          throw invalid('fix the conflicts first');
        }
        sync.stashKept = false;
        sync.stashApplied = false;

        return result(repo, 'done');
    }
  }

  return {
    git_sync_state: ({ project }) => {
      if (!isRepo(state, project)) {
        state.requireKnown(project);

        return structuredClone(NOT_A_REPO);
      }

      return structuredClone(repoOf(project).sync);
    },

    git_fetch: ({ project }) => {
      const repo = repoOf(project);
      if (scenario() === 'offline') {
        throw IpcFailure.from({
          code: 'process',
          message: 'Could not resolve host: github.com',
        });
      }

      repo.sync.lastFetched = new Date().toISOString();

      return structuredClone(repo.sync);
    },

    git_sync_action: ({ project, action }) => act(repoOf(project), action),

    git_switch: ({ project, request }) => {
      const repo = repoOf(project);
      const sync = repo.sync;
      const existing = repo.branches.find((row) => row.name === request.branch);

      if (!isBranchName(request.branch)) {
        throw invalid(`\`${request.branch}\` is not a valid branch name`);
      }

      if (request.create && existing) {
        throw invalid(`a branch named \`${request.branch}\` already exists`);
      }

      if (!request.create && !existing) {
        throw invalid(`there is no branch \`${request.branch}\``);
      }

      if (request.create === 'main' && !sync.main) {
        throw invalid('this project has no main branch');
      }

      if (!request.create && !request.stash && sync.trackedChanges > 0 && request.branch === sync.main?.name) {
        return result(repo, 'blockedByChanges');
      }

      if (request.stash) {
        sync.trackedChanges = 0;
        sync.untracked = 0;
        sync.stashKept = true;
      }

      if (request.create) {
        repo.branches = [
          {
            name: request.branch,
            isCurrent: false,
            remoteOnly: false,
            lastCommitDate: new Date().toISOString(),
          },
          ...repo.branches,
        ];
      } else if (existing) {
        existing.remoteOnly = false;
      }

      const fresh = request.create !== null;
      sync.branch = request.branch;
      sync.upstream = fresh || !sync.remote ? null : `${sync.remote}/${request.branch}`;
      sync.ahead = 0;
      sync.behind = 0;

      if (sync.main) {
        const isCurrent = sync.main.name === request.branch;
        sync.main = {
          ...sync.main,
          isCurrent,
          newCommits: isCurrent || request.create === 'main' ? 0 : repo.mainAhead,
        };
      }

      return result(repo, 'done');
    },

    git_branch_search: ({ project, query, offset, limit }) => {
      const repo = repoOf(project);
      const needle = query.trim().toLowerCase();
      const matches = repo.branches.filter((row) => row.name.toLowerCase().includes(needle));
      const ordered = [...matches.filter((row) => !row.remoteOnly), ...matches.filter((row) => row.remoteOnly)];
      const page: BranchPage = {
        branches: ordered.slice(Math.max(offset, 0), Math.max(offset, 0) + Math.min(Math.max(limit, 1), MAX_SEARCH)).map((row) => ({ ...row, isCurrent: row.name === repo.sync.branch })),
        total: ordered.length,
      };

      return page;
    },

    git_set_main_branch: ({ project, branch }) => {
      const repo = repoOf(project);
      const detected = startingState(scenario()).main;

      if (branch !== null && !isBranchName(branch)) {
        throw invalid(`\`${branch}\` is not a valid branch name`);
      }

      state.mainBranches = [...state.mainBranches.filter((choice) => choice.project !== project), ...(branch === null ? [] : [{ project, branch }])];

      if (branch === null) {
        repo.sync.main = detected && {
          ...detected,
          newCommits: repo.sync.branch === detected.name ? 0 : repo.mainAhead,
          isCurrent: repo.sync.branch === detected.name,
        };
      } else {
        const isCurrent = repo.sync.branch === branch;
        repo.sync.main = {
          name: branch,
          reference: branch,
          source: 'picked',
          newCommits: isCurrent ? 0 : repo.mainAhead,
          isCurrent,
        };
      }

      return structuredClone(repo.sync);
    },
  };
}
