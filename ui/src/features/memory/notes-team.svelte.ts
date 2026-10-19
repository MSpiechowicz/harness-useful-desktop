import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
import type { MemoryNotesPage } from '$lib/ipc/bindings/MemoryNotesPage';
import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { MemoryScopeCounts } from '$lib/ipc/bindings/MemoryScopeCounts';
import type { MemoryScriptsPage } from '$lib/ipc/bindings/MemoryScriptsPage';
import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';
import { readJson, writeJson } from '$lib/utils/persisted';

/** Where the count a banner was dismissed at is kept, per kind: dismissing the notes banner leaves the scripts one alone. */
const DISMISSED_KEYS: Record<MemoryCopyKind, string> = {
  notes: 'harness.notesCopyDismissed',
  scripts: 'harness.scriptsCopyDismissed',
};

function isCount(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0;
}

/**
 * How many entries saved on this computer sit in scopes that read the remote database only (`remote`): they are invisible there, so the view
 * offers to copy or delete them. Zero while no remote database is on, null while that cannot be told (the settings or the page were not read, or
 * the server does not count them).
 */
function strandedIn(remote: MemoryRemote | null, page: { availability: { kind: string }; localCounts: MemoryScopeCounts | null } | null, routing: Record<ScriptScope, MemoryNotesRouting> | null): number | null {
  if (!remote || !page || !routing || page.availability.kind !== 'available') {
    return null;
  }

  if (!remote.enabled) {
    return 0;
  }

  const counts = page.localCounts;

  if (!counts) {
    return null;
  }

  const scopes: ScriptScope[] = ['repo', 'global'];

  return scopes.filter((scope) => routing[scope] === 'remote').reduce((sum, scope) => sum + counts[scope], 0);
}

/** The notes saved on this computer that their scopes' routing hides (see `strandedIn`). */
export function strandedNotes(remote: MemoryRemote | null, page: MemoryNotesPage | null): number | null {
  return strandedIn(remote, page, remote && { repo: remote.repositoryNotes, global: remote.globalNotes });
}

/** The scripts saved on this computer that the routing hides: scripts have one routing for both scopes. */
export function strandedScripts(remote: MemoryRemote | null, page: MemoryScriptsPage | null): number | null {
  return strandedIn(remote, page, remote && { repo: remote.scripts, global: remote.scripts });
}

/** What a view of notes or scripts remembers apart from the server: the hidden filter, and how many stranded entries the user dismissed the banner at. */
class TeamState {
  /** The list shows the remote entries hidden on this computer. */
  showHidden = $state(false);

  /** The count the banner was dismissed at, kept across sessions. The banner returns when the count grows past it. */
  dismissedAt = $state(0);

  constructor(private readonly storageKey: string) {
    this.dismissedAt = readJson(storageKey, 0, isCount);
  }

  dismiss(count: number): void {
    this.dismissedAt = count;
    writeJson(this.storageKey, count);
  }

  /** A count that fell below the remembered one is remembered instead, so only growth from there brings the banner back. */
  follow(count: number): void {
    if (count < this.dismissedAt) {
      this.dismiss(count);
    }
  }

  /** Forgets the filter, e.g. for a project switch or a test. */
  reset(): void {
    this.showHidden = false;
  }
}

export const notesTeam = new TeamState(DISMISSED_KEYS.notes);
export const scriptsTeam = new TeamState(DISMISSED_KEYS.scripts);

/** The team state of one kind. */
export const teamOf = (kind: MemoryCopyKind): TeamState => (kind === 'notes' ? notesTeam : scriptsTeam);
