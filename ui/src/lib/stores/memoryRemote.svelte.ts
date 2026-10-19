import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';
import type { MemoryScopeCounts } from '$lib/ipc/bindings/MemoryScopeCounts';
import type { MemoryRemote } from '$lib/ipc/bindings/MemoryRemote';
import type { MemoryRemoteKind } from '$lib/ipc/bindings/MemoryRemoteKind';
import type { MemoryRemoteTest } from '$lib/ipc/bindings/MemoryRemoteTest';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';

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
}

/** How many notes the file on this computer holds per scope, across all repositories. */
export type LocalCounts = MemoryScopeCounts;

export interface MemoryRemoteProbe {
  kind: MemoryRemoteKind;
  url: string | null;
  token: string | null;
}

/**
 * The team-database setting for project memory. It only ever holds what the backend reports (the URL redacted, never the
 * token); what the user types is kept by the form that owns it and goes straight to the backend, so no secret lands in
 * a store, in storage or in a log. Failures are thrown for the form to show beside its fields.
 */
export class MemoryRemoteStore {
  value = $state<MemoryRemote | null>(null);

  /** Set when the last read failed, so the form can say so instead of showing a stale or empty panel. */
  loadFailed = $state(false);

  constructor(private readonly transport: Transport) {}

  async load(): Promise<void> {
    try {
      this.value = await this.transport.invoke('memory_remote_get', undefined);
      this.loadFailed = false;
    } catch {
      this.loadFailed = true;
    }
  }

  async save(args: MemoryRemoteSave): Promise<MemoryRemote> {
    const saved = await this.transport.invoke('memory_remote_set', args);
    this.value = saved;
    this.loadFailed = false;

    return saved;
  }

  /**
   * The notes saved on this computer, per scope. They ride on any page of the notes list, so the cheapest read is a page of one note.
   * Null when the server does not count them (an older one, or no team database) or the list cannot be read.
   */
  async localCounts(): Promise<LocalCounts | null> {
    try {
      const page = await this.transport.invoke('memory_notes_list', { project: null, offset: 0, limit: 1 });

      return page.localCounts ?? null;
    } catch {
      return null;
    }
  }

  /**
   * How many scripts the file on this computer holds, every scope and repository together, read the same way as the notes (a page of one script).
   * Null when the server does not count them (an older one, or no team database) or the list cannot be read.
   */
  async scriptLocalCounts(): Promise<number | null> {
    try {
      const page = await this.transport.invoke('memory_scripts_list', { project: null, offset: 0, limit: 1 });

      return page.localCounts ? page.localCounts.repo + page.localCounts.global : null;
    } catch {
      return null;
    }
  }

  test(args: MemoryRemoteProbe): Promise<MemoryRemoteTest> {
    return this.transport.invoke('memory_remote_test', args);
  }
}

export const memoryRemote = new MemoryRemoteStore(ipc);
