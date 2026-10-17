import type { MemoryNotesTarget } from '$lib/ipc/bindings/MemoryNotesTarget';
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
  repositoryNotes: MemoryNotesTarget;
  globalNotes: MemoryNotesTarget;
  scripts: MemoryNotesTarget;
  /** Run every script the team database holds without asking. */
  trustTeamScripts: boolean;
}

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

  test(args: MemoryRemoteProbe): Promise<MemoryRemoteTest> {
    return this.transport.invoke('memory_remote_test', args);
  }
}

export const memoryRemote = new MemoryRemoteStore(ipc);
