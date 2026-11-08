import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
import type { MemorySavePlace } from '$lib/ipc/bindings/MemorySavePlace';
import type { MemoryScopeCounts } from '$lib/ipc/bindings/MemoryScopeCounts';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { MemoryRemoteKind } from '$lib/ipc/bindings/MemoryRemoteKind';
import type { MemoryRemoteTest } from '$lib/ipc/bindings/MemoryRemoteTest';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';

import { RemoteDatabaseStore, type RemoteDatabaseProbe } from './remoteDatabase.svelte';

export interface MemoryRemoteSave {
  enabled: boolean;
  kind: MemoryRemoteKind | null;
  /** Null keeps the saved URL. */
  url: string | null;
  /** Null keeps the saved token; an empty string clears it. */
  token: string | null;
  /** Notes may also be read from the team database and saved on this computer (`remoteLocal`). */
  repositoryNotes: MemoryNotesRouting;
  globalNotes: MemoryNotesRouting;
  /** Scripts follow the notes: they may also be read from the team database and saved on this computer (`remoteLocal`). */
  scripts: MemoryNotesRouting;
  /** Run every script the team database holds without asking. */
  trustTeamScripts: boolean;
  /** Share hooks through the team database. Left out keeps the saved choice (a server that cannot keep team hooks has none to change). */
  shareHooks?: boolean;
  /** Run team hooks without local approval. Only saved on while hooks are shared. */
  trustTeamHooks?: boolean;
  /** Save on this computer while the remote database is unreachable. Left out keeps the saved choice (a server that does not know the setting has none to change). */
  offlineFallback?: boolean;
  /** Where new notes and scripts go in "Remote + this computer", for agents and for what the user adds. Left out keeps the saved choice (a server that does not know the setting has none to change). */
  remoteLocalSaves?: MemorySavePlace;
}

/** How many notes the file on this computer holds per scope, across all repositories. */
export type LocalCounts = MemoryScopeCounts;

export type MemoryRemoteProbe = RemoteDatabaseProbe<MemoryRemoteKind>;

/** The team-database setting for project memory, plus what this computer holds that moving to the team database would hide. */
export class MemoryRemoteStore extends RemoteDatabaseStore<MemoryRemote, MemoryRemoteSave, MemoryRemoteProbe, MemoryRemoteTest> {
  constructor(private readonly transport: Transport) {
    super({
      get: () => transport.invoke('memory_remote_get', undefined),
      set: (args) => transport.invoke('memory_remote_set', args),
      test: (args) => transport.invoke('memory_remote_test', args),
    });
  }

  /**
   * The notes saved on this computer, per scope. They ride on any page of the notes list, so the cheapest read is a page of one note.
   * Null when the server does not count them (an older one, or no team database), the list cannot be read, or the page is the copy kept
   * while the remote database is unreachable: what it counts as saved here may not be all of it, so nothing is claimed or offered from it.
   */
  async localCounts(): Promise<LocalCounts | null> {
    try {
      const page = await this.transport.invoke('memory_notes_list', { project: null, offset: 0, limit: 1 });

      return page.offline ? null : (page.localCounts ?? null);
    } catch {
      return null;
    }
  }

  /**
   * How many scripts the file on this computer holds, every scope and repository together, read the same way as the notes (a page of one script).
   * Null when the server does not count them (an older one, or no team database), the list cannot be read, or the page is the copy kept while the
   * remote database is unreachable (see `localCounts`).
   */
  async scriptLocalCounts(): Promise<number | null> {
    try {
      const page = await this.transport.invoke('memory_scripts_list', { project: null, offset: 0, limit: 1 });

      return page.offline || !page.localCounts ? null : page.localCounts.repo + page.localCounts.global;
    } catch {
      return null;
    }
  }
}

export const memoryRemote = new MemoryRemoteStore(ipc);
