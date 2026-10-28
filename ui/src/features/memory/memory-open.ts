import type { ProjectScope } from '$lib/ipc/bindings/ProjectScope';
import type { ScriptSummary } from '$lib/ipc/bindings/ScriptSummary';
import { ipc } from '$lib/ipc/client';
import { ALL_REPOS, GLOBAL_ONLY, reachArgs } from '$lib/stores/memory-reach';
import { nav } from '$lib/stores/nav.svelte';
import { projectScopes } from '$lib/stores/project-scopes.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { bare } from './scope-target';

/** Where a note or script lives: one repository, or everywhere. */
type Scope = 'repo' | 'global';

/**
 * The Notes and Scripts views read one reach, shared by both. Whatever the thing being opened lives in must be inside it, or
 * the panel would open on an entry the table does not list: a reach of one repository (or none the app can name) is widened
 * to every repository, and the global-only reach too when the thing belongs to a repository. Kind, place and the other filters stay.
 */
function reachTo(scope: Scope | undefined): void {
  const reach = viewState.memoryReach;
  const specific = reach !== ALL_REPOS && reach !== GLOBAL_ONLY;

  if (specific || (reach === GLOBAL_ONLY && scope !== 'global')) {
    viewState.memoryReach = ALL_REPOS;
  }
}

/** Opens a note in the Notes view, from a card in the chat. */
export function showNote(id: number, scope?: Scope): void {
  reachTo(scope);
  viewState.noteId = id;
  nav.go('notes');
}

/**
 * Opens a saved script in the Scripts view: by its id when the chat knows it, else by the name a search listed. A name is looked up
 * among the scripts of every repository (an exact match), the script of the chat's project (`projectPath`) first, then a global
 * one, then any other, and a script that cannot be found leaves the view searching for the name.
 */
export async function showScript(target: number | string, projectPath: string | null = null): Promise<void> {
  reachTo(undefined);

  if (typeof target === 'number') {
    viewState.scriptId = target;
    nav.go('scripts');

    return;
  }

  const id = await idOfScript(target, projectPath);

  if (id === null) {
    viewState.scriptsQuery = target;
  } else {
    viewState.scriptId = id;
  }

  nav.go('scripts');
}

/** The repository key Harness Memory knows the project by; null when it has none or the projects cannot be read. */
async function repoKeyOf(projectPath: string | null): Promise<string | null> {
  if (projectPath === null) {
    return null;
  }

  try {
    const scopes: ProjectScope[] = projectScopes.data ?? (await ipc.invoke('project_scopes', undefined));

    return scopes.find((scope) => scope.path === projectPath)?.key ?? null;
  } catch {
    return null;
  }
}

/** Of scripts of one name, the one the chat means: its project's own, else a global one, else the first. */
function preferred(scripts: ScriptSummary[], repoKey: string | null): ScriptSummary | undefined {
  const own = repoKey === null ? undefined : scripts.find((script) => script.scope === 'repo' && script.repo !== null && bare(script.repo) === bare(repoKey));

  return own ?? scripts.find((script) => script.scope === 'global') ?? scripts[0];
}

async function idOfScript(name: string, projectPath: string | null): Promise<number | null> {
  try {
    const page = await ipc.invoke('memory_scripts_list', { ...reachArgs(ALL_REPOS), offset: 0, limit: 50, query: name });
    if (page.availability.kind !== 'available') {
      return null;
    }

    const named = page.scripts.filter((script) => script.name === name);

    return preferred(named, named.length > 1 ? await repoKeyOf(projectPath) : null)?.id ?? null;
  } catch {
    return null;
  }
}
