import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';

import { projectChoice, type ScopeTarget, targetValue } from '../scope/scope-options';
import type { ScopedDoc } from './doc-scope';

/** Where a new document starts out in its editor: the project on this computer the Scope filter shows, else Global (also for All and for a repository only the team has). */
export function newDocTarget(choice: string, projects: ProjectScope[]): ScopeTarget {
  const chosen = projects.find((project) => projectChoice(project) === choice);

  return chosen ? { kind: 'repo', path: chosen.path } : { kind: 'global' };
}

/**
 * Where a document that exists applies, as its editor shows it: the project on this computer with its root, else the root itself
 * (a project that left the recent ones), which the field lists as that folder.
 */
export function docTarget(doc: ScopedDoc, projects: ProjectScope[]): ScopeTarget {
  if (doc.scope !== 'project' || doc.root === null) {
    return { kind: 'global' };
  }

  const project = projects.find((candidate) => candidate.root === doc.root);

  return { kind: 'repo', path: project?.path ?? doc.root };
}

/** The project root a target stands for: the root of its project on this computer, else the folder itself. Null for Global and for a project whose folder is gone. */
export function targetRoot(target: ScopeTarget, projects: ProjectScope[]): string | null {
  if (target.kind === 'global') {
    return null;
  }

  const project = projects.find((candidate) => candidate.path === target.path);

  return project ? project.root : target.path;
}

/** Whether two targets are one scope. */
export function sameTarget(a: ScopeTarget, b: ScopeTarget): boolean {
  return targetValue(a) === targetValue(b);
}

/** How many documents of this computer (not the team's) a target already holds, for the most a scope can hold. */
export function heldIn<V>(views: V[], target: ScopeTarget, projects: ProjectScope[], docOf: (view: V) => ScopedDoc): number {
  const root = targetRoot(target, projects);

  return views.filter((view) => {
    const doc = docOf(view);

    if (doc.origin !== 'local') {
      return false;
    }

    return target.kind === 'global' ? doc.scope === 'global' : doc.scope === 'project' && root !== null && doc.root === root;
  }).length;
}

/**
 * Whether a document that exists stays in its scope when saved for `target`: its own stored root against the root of the target, read
 * from the projects as they are now, so it does not matter when they were read. A file that is not a document never stays.
 */
export function staysIn(doc: ScopedDoc, target: ScopeTarget, projects: ProjectScope[]): boolean {
  if (doc.scope === 'global') {
    return target.kind === 'global';
  }

  return doc.scope === 'project' && doc.root !== null && target.kind === 'repo' && targetRoot(target, projects) === doc.root;
}
