import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
import { ALL_REPOS, GLOBAL_ONLY, LOCAL_PREFIX, reachKey, repoReach } from '$lib/stores/memory-reach';

/**
 * What a Scope filter is set to: every scope (`all`), the global entries (`global`), or one repository (`repo:<key>` by the key
 * Harness Memory gives it). It is the reach the Memory views already key their loaders on, so the two stay one string.
 * A project whose key is not known (Harness Memory is missing) is `repo:local:<path>`: it has no key to filter by.
 */
export type ScopeChoice = 'all' | 'global' | `repo:${string}`;

/** A repository the data source knows, by its key, as the Memory repository lists give it. */
export interface KnownRepo {
  key: string;
  /** The repository as the source names it: `github.com/acme/widget`, or the folder path of a `path:` key. */
  label: string;
  count: number;
}

/** One entry of a Scope filter. */
export interface ScopeOption {
  value: ScopeChoice;
  label: string;
  /** A quiet second line: `owner/repo` under a folder name, or "Remote only". */
  hint?: string;
  count?: number;
  /** The repository is not on this computer, so nothing can be saved to it. */
  remoteOnly: boolean;
  /** A project on this computer without a repository key: it can be chosen, but not filtered by (the data source knows no key for it). */
  keyless: boolean;
  /** Set for a project on this computer: the folder and the root hooks and routines store. */
  path?: string;
  root?: string;
}

/** The texts of the entries that are not repositories. */
export interface ScopeLabels {
  all: string;
  global: string;
  remoteOnly: string;
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

/** `owner/repo` of a repository shown as `host/owner/repo`; a folder (`path:` key) as it is. */
export function shortRepoName(label: string): string {
  const parts = label.split('/');

  return parts.length > 2 && !label.startsWith('/') ? parts.slice(1).join('/') : label;
}

/** The choice of the project: its repository key when it has one, else its folder. */
export function projectChoice(project: Pick<ProjectScope, 'path' | 'key'>): ScopeChoice {
  return repoReach(project.key ?? `${LOCAL_PREFIX}${project.path}`) as ScopeChoice;
}

/** The repository key a choice filters by; null for `all`, `global` and a project without a key. */
export function choiceKey(choice: ScopeChoice): string | null {
  return reachKey(choice);
}

/** The label of a repository key: `github.com/acme/widget` for `git:github.com/acme/widget`, the folder for `path:/work/widget`. */
function keyLabel(key: string): string {
  return key.replace(/^(git|path):/, '');
}

/** The `owner/repo` shown under a project's folder name; none for a folder that is no repository or has no key. */
function projectHint(project: ProjectScope): string | undefined {
  if (project.key === null || !project.key.startsWith('git:')) {
    return undefined;
  }

  return shortRepoName(keyLabel(project.key));
}

function byLabel(a: { label: string }, b: { label: string }): number {
  return collator.compare(a.label, b.label);
}

/** The entries with the folder (`pathOf`) as their hint where the label and hint alone would not tell them apart. */
function toldApart<T extends { label: string; hint?: string }>(entries: T[], pathOf: (entry: T) => string | undefined): T[] {
  const looks = (entry: T) => `${entry.label}\n${entry.hint ?? ''}`;
  const seen = new Map<string, number>();

  for (const entry of entries) {
    seen.set(looks(entry), (seen.get(looks(entry)) ?? 0) + 1);
  }

  return entries.map((entry) => ((seen.get(looks(entry)) ?? 0) > 1 ? { ...entry, hint: pathOf(entry) ?? entry.hint } : entry));
}

/** The projects on this computer, one per choice (two folders of the same repository are one entry, the first listed wins). */
function distinctProjects(projects: ProjectScope[]): ProjectScope[] {
  const seen = new Set<ScopeChoice>();

  return projects.filter((project) => {
    const choice = projectChoice(project);

    if (seen.has(choice)) {
      return false;
    }

    seen.add(choice);

    return true;
  });
}

interface BuildInput {
  /** The repositories the user works with on this computer. */
  projects: ProjectScope[];
  /** The repositories the data source knows with their entry counts (local ones too, merged by key). Null while they are not known. */
  remote: KnownRepo[] | null;
  /** Counts the source of the entries has for a choice, where `remote` does not give them (the rows of a list). */
  counts?: ReadonlyMap<ScopeChoice, number> | null;
  /** The count of the global entries and of all of them; left out while not known. */
  globalCount?: number;
  allCount?: number;
  labels: ScopeLabels;
}

/**
 * The entries of a Scope filter: All with the total, the projects on this computer by name (always listed, even with none),
 * the repositories only the source knows by name, and Global last. A project and a source repository with one key are one entry.
 */
export function buildScopeOptions({ projects, remote, counts = null, globalCount, allCount, labels }: BuildInput): ScopeOption[] {
  const known = new Map((remote ?? []).map((repo) => [repo.key, repo]));
  const local = distinctProjects(projects);
  const localKeys = new Set(local.flatMap((project) => (project.key === null ? [] : [project.key])));
  const sourced = remote !== null || counts !== null;

  const countOf = (choice: ScopeChoice, key: string | null): number | undefined => {
    const own = counts?.get(choice) ?? (key === null ? undefined : known.get(key)?.count);

    return own ?? (sourced ? 0 : undefined);
  };

  const here = local
    .map<ScopeOption>((project) => {
      const value = projectChoice(project);

      return {
        value,
        label: project.name,
        hint: projectHint(project),
        count: countOf(value, project.key),
        remoteOnly: false,
        keyless: project.key === null,
        path: project.path,
        root: project.root ?? undefined,
      };
    })
    .sort(byLabel);
  const shown = toldApart(here, (option) => option.path);

  const away = (remote ?? [])
    .filter((repo) => !localKeys.has(repo.key))
    .map<ScopeOption>((repo) => ({ value: repoReach(repo.key) as ScopeChoice, label: shortRepoName(repo.label), hint: labels.remoteOnly, count: repo.count, remoteOnly: true, keyless: false }))
    .sort(byLabel);

  return [
    { value: ALL_REPOS as ScopeChoice, label: labels.all, count: allCount, remoteOnly: false, keyless: false },
    ...shown,
    ...away,
    { value: GLOBAL_ONLY as ScopeChoice, label: labels.global, count: globalCount, remoteOnly: false, keyless: false },
  ];
}

/** What a row of a list says about where it applies: global, or a repository by its git root and its Memory key (either may be unknown). */
export interface ScopeRow {
  global: boolean;
  root: string | null;
  key: string | null;
}

/** Whether a row belongs under a choice: any row under `all`, the global ones under `global`, and under a repository the rows with its key or in the root of a project on this computer that has it. */
export function matchesScope(choice: ScopeChoice, row: ScopeRow, projects: ProjectScope[]): boolean {
  if (choice === 'all') {
    return true;
  }

  if (choice === 'global') {
    return row.global;
  }

  if (row.global) {
    return false;
  }

  const key = choiceKey(choice);

  if (key !== null && row.key === key) {
    return true;
  }

  const mine = projects.filter((project) => projectChoice(project) === choice);

  return row.root !== null && mine.some((project) => project.root === row.root);
}

/** What an editor's Applies to field is set to: the global scope, or the project at a folder on this computer. */
export type ScopeTarget = { kind: 'global' } | { kind: 'repo'; path: string };

/** One entry of an Applies to field. */
export interface TargetOption {
  value: string;
  label: string;
  hint?: string;
}

const GLOBAL_VALUE = 'global';
const REPO_VALUE = 'repo:';

/** The option value of a target, the string a menu keys on. */
export function targetValue(target: ScopeTarget): string {
  return target.kind === 'global' ? GLOBAL_VALUE : `${REPO_VALUE}${target.path}`;
}

/** The target an option value stands for. */
export function targetOf(value: string): ScopeTarget {
  return value.startsWith(REPO_VALUE) ? { kind: 'repo', path: value.slice(REPO_VALUE.length) } : { kind: 'global' };
}

/** The folder name of a path, for a project the field has to show but is not among the projects on this computer any more. */
function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/**
 * The entries of an Applies to field: Global, then the projects on this computer by name with `owner/repo` under the name.
 * Nothing is saved to a repository only the source knows, and nothing to a project whose folder is gone (a save has to find its
 * repository there). Projects that would look alike (two clones of one repository, two folders of one name) are told apart by their
 * folder. A `current` target that is no project of the list (one that left the recent projects) is listed too by its folder,
 * so the field never claims another scope than the one it holds.
 */
export function buildTargetOptions(projects: ProjectScope[], globalLabel: string, current?: ScopeTarget): TargetOption[] {
  const here = projects
    .filter((project) => project.root !== null || (current?.kind === 'repo' && current.path === project.path))
    .map<TargetOption>((project) => ({ value: targetValue({ kind: 'repo', path: project.path }), label: project.name, hint: projectHint(project) }))
    .sort(byLabel);

  const options = [{ value: GLOBAL_VALUE, label: globalLabel }, ...toldApart(here, (option) => option.value.slice(REPO_VALUE.length))];

  if (current && !options.some((option) => option.value === targetValue(current)) && current.kind === 'repo') {
    options.push({ value: targetValue(current), label: folderName(current.path), hint: current.path });
  }

  return options;
}
