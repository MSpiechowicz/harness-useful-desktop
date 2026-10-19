import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
import type { LocalCounts } from '$lib/stores/memoryRemote.svelte';

/** The three things whose place the user chooses: the two scopes of notes, and the scripts. */
export type RoutingScope = 'repository' | 'global' | 'scripts';

/** Where each scope goes: the part of the saved settings that decides what the user sees. */
export interface Routes {
  repositoryNotes: MemoryNotesRouting;
  globalNotes: MemoryNotesRouting;
  scripts: MemoryNotesRouting;
}

/** What this computer holds that a move to the remote database would hide: the notes per scope, and the scripts of every scope together. Null when not known. */
export interface LocalHoldings {
  notes: LocalCounts | null;
  scripts: number | null;
}

/**
 * What one scope's change does to what the user sees.
 * - `hideTeam`: the remote entries of the scope stop showing on this computer. They stay in the remote database.
 * - `showTeam`: the remote entries appear, read-only, and new ones are still saved here.
 * - `saveHere`: nothing is hidden. New ones are saved on this computer and the remote ones become read-only here.
 * - `hideLocal`: the entries saved on this computer stop showing (`count` of them, null when not known). Copying them to the remote database comes next.
 */
export type RoutingEffectKind = 'hideTeam' | 'showTeam' | 'saveHere' | 'hideLocal';

export interface RoutingEffect {
  scope: RoutingScope;
  kind: RoutingEffectKind;
  /** Only for `hideLocal`. */
  count: number | null;
}

const SCOPES: readonly RoutingScope[] = ['repository', 'global', 'scripts'];

function routeOf(routes: Routes, scope: RoutingScope): MemoryNotesRouting {
  if (scope === 'scripts') {
    return routes.scripts;
  }

  return scope === 'repository' ? routes.repositoryNotes : routes.globalNotes;
}

function localCount(holdings: LocalHoldings, scope: RoutingScope): number | null {
  if (scope === 'scripts') {
    return holdings.scripts;
  }

  if (holdings.notes === null) {
    return null;
  }

  return scope === 'repository' ? holdings.notes.repo : holdings.notes.global;
}

/** Whether the remote database is read in this place. */
export function readsRemote(routing: MemoryNotesRouting): boolean {
  return routing !== 'local';
}

/** The one thing a move from `from` to `to` does to the scope, or null when it changes nothing the user sees. */
function kindOf(from: MemoryNotesRouting, to: MemoryNotesRouting): RoutingEffectKind | null {
  if (from === to) {
    return null;
  }

  if (to === 'remote') {
    return 'hideLocal';
  }

  if (to === 'local') {
    return 'hideTeam';
  }

  return from === 'remote' ? 'saveHere' : 'showTeam';
}

/**
 * What saving `to` instead of `from` changes, per scope that changes, in the order of the form. Moving to the remote database
 * with nothing saved on this computer hides nothing, so it says nothing (the counts decide, and unknown counts say it).
 */
export function routingEffects(from: Routes, to: Routes, holdings: LocalHoldings): RoutingEffect[] {
  const effects: RoutingEffect[] = [];

  for (const scope of SCOPES) {
    const kind = kindOf(routeOf(from, scope), routeOf(to, scope));

    if (kind === null) {
      continue;
    }

    const count = kind === 'hideLocal' ? localCount(holdings, scope) : null;

    if (kind === 'hideLocal' && count === 0) {
      continue;
    }

    effects.push({ scope, kind, count });
  }

  return effects;
}

/** Whether the form changes where a scope goes. */
export function routesChanged(from: Routes, to: Routes): boolean {
  return SCOPES.some((scope) => routeOf(from, scope) !== routeOf(to, scope));
}

/**
 * The scopes the remote database alone holds after a save that did not hold them alone before (`wasOn` false is a first connection,
 * where every scope was local). What is saved on this computer for these is what to offer to copy.
 */
export function scopesMovedToTeam(before: Routes, wasOn: boolean, after: Routes): RoutingScope[] {
  return SCOPES.filter((scope) => routeOf(after, scope) === 'remote' && (!wasOn || routeOf(before, scope) !== 'remote'));
}

/** What to offer to copy, in the order of the dialogs: the notes of the moved scopes first, then the scripts, each only when this computer holds some. */
export function copyOffers(moved: readonly RoutingScope[], holdings: LocalHoldings): MemoryCopyKind[] {
  const offers: MemoryCopyKind[] = [];
  const noteScopes = moved.filter((scope) => scope !== 'scripts');

  if (noteScopes.some((scope) => (localCount(holdings, scope) ?? 0) > 0)) {
    offers.push('notes');
  }

  if (moved.includes('scripts') && (holdings.scripts ?? 0) > 0) {
    offers.push('scripts');
  }

  return offers;
}
