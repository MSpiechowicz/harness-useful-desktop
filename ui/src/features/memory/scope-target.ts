import { buildTargetOptions, projectChoice, type ScopeTarget, targetValue } from '$lib/components/scope/scope-options';
import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
import { GLOBAL_ONLY } from '$lib/stores/memory-reach';

import type { Problem } from './editor-fields.svelte';

/** Where an item applies, as the lists give it: global, or a repository by its key (`github.com/o/r`, `path:/abs`). */
interface Placed {
  scope: 'repo' | 'global';
  repo: string | null;
}

/** A repository key without the `git:` Harness Memory puts before a remote, which its entries leave out. */
export function bare(key: string): string {
  return key.replace(/^git:/, '');
}

/** The project on this computer a repository key belongs to, if any. */
function projectOfRepo(repo: string, projects: ProjectScope[]): ProjectScope | undefined {
  return projects.find((candidate) => candidate.key !== null && bare(candidate.key) === bare(repo));
}

/**
 * Where a new note or script is kept unless the user says otherwise: where the Scope filter looks (Global, or a project on this computer),
 * else the open project, else Global.
 */
export function defaultTarget(reach: string, projects: ProjectScope[], activePath: string | null): ScopeTarget {
  if (reach === GLOBAL_ONLY) {
    return { kind: 'global' };
  }

  const chosen = projects.find((candidate) => projectChoice(candidate) === reach);

  if (chosen) {
    return { kind: 'repo', path: chosen.path };
  }

  return activePath === null ? { kind: 'global' } : { kind: 'repo', path: activePath };
}

/**
 * The scope an item that exists is shown with. A repository on this computer is its project, any other repository (one only the
 * team database has) is shown by its key, since the field only displays it.
 */
export function targetOfItem(item: Placed, projects: ProjectScope[]): ScopeTarget {
  if (item.scope === 'global' || item.repo === null) {
    return { kind: 'global' };
  }

  return { kind: 'repo', path: projectOfRepo(item.repo, projects)?.path ?? item.repo };
}

/**
 * The project an edit is saved from: the project of the item's repository when it is on this computer, else the open one.
 * (The server finds the item by its id, the project only authorizes the write.)
 */
export function editProject(item: Placed, projects: ProjectScope[], activePath: string | null): string | null {
  return (item.repo === null ? undefined : projectOfRepo(item.repo, projects)?.path) ?? activePath;
}

/** The project a new item is saved in: the chosen folder, none for Global. */
export function projectOfTarget(target: ScopeTarget): string | null {
  return target.kind === 'repo' ? target.path : null;
}

/** The name the field shows the target by (a project's name, else its folder). */
export function targetLabel(projects: ProjectScope[], target: ScopeTarget, globalLabel: string): string {
  const value = targetValue(target);

  return buildTargetOptions(projects, globalLabel, target).find((option) => option.value === value)?.label ?? globalLabel;
}

/** What is wrong with where a new item applies: a repository needs a folder to be kept for. An item that exists keeps its scope, so nothing is checked. */
export function targetProblems(target: ScopeTarget, editing: boolean): Problem<'scope'>[] {
  return !editing && target.kind === 'repo' && target.path === '' ? [{ field: 'scope', message: 'memory.editor.scope.needsProject', required: false }] : [];
}
