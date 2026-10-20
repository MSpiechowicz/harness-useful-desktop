import type { HookOrigin } from '$lib/ipc/bindings/HookOrigin';
import type { HookScope } from '$lib/ipc/bindings/HookScope';
import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import { buildScopeOptions, type KnownRepo, matchesScope, projectChoice, type ScopeChoice, type ScopeLabels, type ScopeOption, type ScopeRow, shortRepoName } from '../scope/scope-options';

/** What a documents list (hooks, routines) knows of where one of its rows applies. `scope` is that of the parsed body, null for a file that is not a document. */
export interface ScopedDoc {
  origin: HookOrigin;
  scope: HookScope | null;
  /** The project root of a project document, when known (see `HookView.root`). */
  root: string | null;
  /** The repository key of a project document, when known (see `HookView.repoKey`). */
  repoKey: string | null;
}

/** What the Scope cell of a row shows: the global scope, one repository by name, or an unknown scope (a file that is not a document). */
export type DocScope = { kind: 'global' } | { kind: 'repo'; label: string } | { kind: 'unknown' };

/** The row as a Scope filter matches it. A file that is not a document is neither global nor of a repository: only All shows it. */
export function scopeRow(doc: ScopedDoc): ScopeRow {
  return { global: doc.scope === 'global', root: doc.scope === 'project' ? doc.root : null, key: doc.scope === 'project' ? doc.repoKey : null };
}

/** The project on this computer a row belongs to: the one with its key, else the one with its root. */
function projectOf(doc: ScopedDoc, projects: ProjectScope[]): ProjectScope | undefined {
  return projects.find((project) => doc.repoKey !== null && project.key === doc.repoKey) ?? projects.find((project) => doc.root !== null && project.root === doc.root);
}

/** The folder name of a path. */
function folderName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

/** What the Scope cell of a row says: Global, the name of its project on this computer, else `owner/repo` of its key or the folder of its root. */
export function docScope(doc: ScopedDoc, projects: ProjectScope[]): DocScope {
  if (doc.scope === null) {
    return { kind: 'unknown' };
  }

  if (doc.scope === 'global') {
    return { kind: 'global' };
  }

  const project = projectOf(doc, projects);

  if (project) {
    return { kind: 'repo', label: project.name };
  }

  if (doc.repoKey !== null) {
    return { kind: 'repo', label: shortRepoName(doc.repoKey.replace(/^(git|path):/, '')) };
  }

  return doc.root === null ? { kind: 'unknown' } : { kind: 'repo', label: folderName(doc.root) };
}

/**
 * The entries of the Scope filter of a documents list, counted over its rows: All, the projects on this computer (with none too),
 * the repositories only team documents name ("Remote only"), and Global. A row of an unknown scope counts under All only.
 */
export function docScopeOptions(docs: ScopedDoc[], projects: ProjectScope[], labels: ScopeLabels): ScopeOption[] {
  const counts = new Map<ScopeChoice, number>();
  const away = new Map<string, number>();

  for (const project of projects) {
    const choice = projectChoice(project);

    counts.set(choice, docs.filter((doc) => matchesScope(choice, scopeRow(doc), projects)).length);
  }

  for (const doc of docs) {
    if (doc.scope === 'project' && doc.repoKey !== null && projectOf(doc, projects) === undefined) {
      away.set(doc.repoKey, (away.get(doc.repoKey) ?? 0) + 1);
    }
  }

  const remote: KnownRepo[] = [...away].map(([key, count]) => ({ key, label: key.replace(/^(git|path):/, ''), count }));

  return buildScopeOptions({
    projects,
    remote,
    counts,
    globalCount: docs.filter((doc) => doc.scope === 'global').length,
    allCount: docs.length,
    labels,
  });
}

/** The rows a Scope filter choice keeps. */
export function inScope<V>(views: V[], choice: ScopeChoice, docOf: (view: V) => ScopedDoc, projects: ProjectScope[]): V[] {
  return choice === 'all' ? views : views.filter((view) => matchesScope(choice, scopeRow(docOf(view)), projects));
}
