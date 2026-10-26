import type { CommandArgs, CommandName } from '../commands';
import { fixtures } from './fixtures';

const PROJECT = fixtures.recentProjects[0].path;

/** One valid call per branch sync command, for the "implements" check of `mock-backend.test.ts`. */
export const BRANCH_SYNC_ARGS: { [K in Extract<CommandName, 'git_sync_state' | 'git_fetch' | 'git_sync_action' | 'git_switch' | 'git_branch_search' | 'git_set_main_branch'>]: CommandArgs<K> } = {
  git_sync_state: { project: PROJECT },
  git_fetch: { project: PROJECT, background: true },
  git_sync_action: { project: PROJECT, action: 'push' },
  git_switch: { project: PROJECT, request: { branch: 'main', create: null, stash: false } },
  git_branch_search: { project: PROJECT, query: 'feat', limit: 20 },
  git_set_main_branch: { project: PROJECT, branch: null },
};
