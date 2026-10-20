import type { MemoryRepos } from '$lib/ipc/bindings/MemoryRepos';
import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';

/**
 * Which notes or scripts the Memory views read, as one string the loaders can key on: every repository (`all`), the global ones
 * alone (`global`), or one repository by the key Harness Memory gives it (`repo:git:github.com/acme/widget`).
 */
export type Reach = string;

export const ALL_REPOS: Reach = 'all';
export const GLOBAL_ONLY: Reach = 'global';

const REPO_PREFIX = 'repo:';

/** The stand-in key of a project on this computer that Harness Memory knows no repository key for (`repo:local:<folder>`): it names no repository to read. */
export const LOCAL_PREFIX = 'local:';

/** The reach of one repository, by its key. */
export function repoReach(key: string): Reach {
  return `${REPO_PREFIX}${key}`;
}

/** Whether a reach is a project on this computer without a repository key. It reads nothing by itself (see `reachArgs`), so nothing may be deleted by it. */
export function isKeyless(reach: Reach): boolean {
  return reach.startsWith(`${REPO_PREFIX}${LOCAL_PREFIX}`);
}

/** The repository key of a reach of one repository; null for any other, and for a project without a key (nothing to read it by). */
export function reachKey(reach: Reach): string | null {
  const key = reach.startsWith(REPO_PREFIX) ? reach.slice(REPO_PREFIX.length) : null;

  return key === null || key.startsWith(LOCAL_PREFIX) ? null : key;
}

/** What the notes and scripts commands take to read `reach`: no project, and the repositories when there are any. */
export function reachArgs(reach: Reach): { project: null; repos?: MemoryRepos } {
  if (reach === ALL_REPOS) {
    return { project: null, repos: { kind: 'all' } };
  }

  const key = reachKey(reach);

  return key === null ? { project: null } : { project: null, repos: { kind: 'repo', key } };
}

/** The scope filter a reach asks of a server that combines filters: one repository's own entries only, or the global ones only; none for every repository. */
export function reachScope(reach: Reach): ScriptScope | null {
  if (reach === GLOBAL_ONLY) {
    return 'global';
  }

  return reachKey(reach) === null ? null : 'repo';
}
