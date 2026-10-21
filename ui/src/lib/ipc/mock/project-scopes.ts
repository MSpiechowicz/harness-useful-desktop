import type { ProjectInfo } from '../bindings/ProjectInfo';
import type { ProjectScope } from '../bindings/ProjectScope';
import { fixtures } from './fixtures';
import type { TeamMode } from './team-preview';

/** The repository of the demo project (the first recent one): the sample notes and scripts and the team hooks and routines of the previews are kept under it. */
export const DEMO_REPO = 'git:github.com/useful/harness-desktop';

/** A repository only the team has: no folder on this computer is a clone of it, so its team hooks and routines are matched by key only (listed with `all`). */
export const REMOTE_ONLY_REPO = 'git:github.com/acme/website';

/** The key Harness Memory gives each mock git repository by its remote. Any other folder is keyed by its path, so the research notes folder has a key and no entries anywhere. */
const REMOTE_KEYS: Record<string, string> = {
  [fixtures.recentProjects[0].path]: DEMO_REPO,
  '/home/demo/Projekty/website': 'git:github.com/demo/website',
  '/home/demo/Projekty/harness-useful-desktop': 'git:github.com/demo/harness-useful-desktop',
};

function keyOf(root: string): string {
  return REMOTE_KEYS[root] ?? `path:${root}`;
}

/** What `project_scopes` answers for one project: no root for a folder that is gone, no key while Harness Memory cannot be asked. */
export function scopeOf(project: ProjectInfo, memoryUp: boolean): ProjectScope {
  const root = project.exists ? project.path : null;

  return { path: project.path, name: project.name, root, key: root !== null && memoryUp ? keyOf(root) : null };
}

/** The `root` and `repoKey` of a local listing row: the key comes from the team refresh, so only while documents are shared (a repository without origin keeps its path key). */
export function localPlace(doc: { scope: 'global' | 'project'; project: string | null } | null, team: TeamMode | null): { root: string | null; repoKey: string | null } {
  const root = doc?.scope === 'project' ? doc.project : null;

  if (root === null || team === null) {
    return { root, repoKey: null };
  }

  return { root, repoKey: team.shareable ? keyOf(root) : `path:${root}` };
}

/** The `root` and `repoKey` of a team listing row of repository `repoKey`: the demo project is the only folder this session confirmed. */
export function teamPlace(repoKey: string | null): { root: string | null; repoKey: string | null } {
  const root = repoKey !== null && repoKey === DEMO_REPO ? fixtures.recentProjects[0].path : null;

  return { root, repoKey };
}
