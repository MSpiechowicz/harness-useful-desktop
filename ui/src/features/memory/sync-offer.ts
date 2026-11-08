import type { AppPrefs } from '$lib/ipc/bindings/AppPrefs';
import type { MemoryCopyKind } from '$lib/ipc/bindings/MemoryCopyKind';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { NotesCopyMode } from '$lib/ipc/bindings/NotesCopyMode';

/** What a list page says about whether the remote database can be used now. */
interface PageState {
  availability: { kind: string };
  offline: boolean;
}

/** Whether the notes or the scripts have a scope that reads the remote database and this computer together, and the installed Harness Memory can keep that and copy between the two. */
function mixesPlaces(kind: MemoryCopyKind, remote: MemoryRemote): boolean {
  if (kind === 'scripts') {
    return remote.scriptsSupported && remote.scriptsTeamAndLocalSupported && remote.scriptsSyncSupported && remote.scripts === 'remoteLocal';
  }

  return remote.teamAndLocalSupported && remote.notesSyncSupported && (remote.repositoryNotes === 'remoteLocal' || remote.globalNotes === 'remoteLocal');
}

/**
 * The ways the Sync button copies the notes or the scripts, in the order the dialog offers them. None while the button is not offered: nothing is
 * read from both places, the installed Harness Memory cannot copy between them, the remote database is not saved or cannot be reached now (the list is the copy kept on this computer then), or the
 * user turned neither copy on in Settings. Nothing is offered until the settings, the preferences and the list are read.
 */
export function syncDirections(kind: MemoryCopyKind, remote: MemoryRemote | null, page: PageState | null, prefs: Pick<AppPrefs, 'memoryCopyToRemote' | 'memoryCopyToLocal'> | null): NotesCopyMode[] {
  if (!remote || !page || !prefs || !remote.enabled || page.availability.kind !== 'available' || page.offline || !mixesPlaces(kind, remote)) {
    return [];
  }

  const directions: NotesCopyMode[] = [];

  if (prefs.memoryCopyToRemote) {
    directions.push('toRemote');
  }

  if (prefs.memoryCopyToLocal) {
    directions.push('toLocal');
  }

  return directions;
}
