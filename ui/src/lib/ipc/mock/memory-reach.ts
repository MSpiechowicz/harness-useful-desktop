import type { MemoryRepoEntry } from '../bindings/MemoryRepoEntry';
import type { MemoryRepos } from '../bindings/MemoryRepos';
import type { MemoryReposPage } from '../bindings/MemoryReposPage';

/** What a notes or scripts command reads: a project's repository, or the repositories named in `repos`. */
export interface Reach {
  project: string | null;
  repos?: MemoryRepos;
}

/** An entry that has a scope and, when it is a repository one, the repository it belongs to. */
interface Placed {
  scope: 'repo' | 'global';
  repo: string | null;
}

/** Whether `entry` is in what `reach` reads, as the real server decides: global entries always, repository ones for every repository, the one named, or a project's. */
export function reaches(entry: Placed, { project, repos }: Reach): boolean {
  if (entry.scope === 'global' || repos?.kind === 'all') {
    return true;
  }

  if (repos?.kind === 'repo') {
    return entry.repo === repos.key.replace(/^git:/, '');
  }

  return project !== null;
}

/** The repositories the entries are in, by label, and how many global ones there are: what `memory_repos_list` answers. */
export function reposPage(entries: Placed[]): MemoryReposPage {
  const counts = new Map<string, number>();

  for (const entry of entries) {
    if (entry.scope === 'repo' && entry.repo !== null) {
      counts.set(entry.repo, (counts.get(entry.repo) ?? 0) + 1);
    }
  }

  const repos: MemoryRepoEntry[] = [...counts]
    .map(([label, count]) => ({ key: `git:${label}`, label, count }))
    .sort((a, b) => a.label.localeCompare(b.label));

  return { availability: { kind: 'available' }, repos, global: entries.filter((entry) => entry.scope === 'global').length };
}
