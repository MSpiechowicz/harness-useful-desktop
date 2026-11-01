import type { KnowledgeCheck } from '$lib/ipc/bindings/KnowledgeCheck';
import type { KnowledgeRemote } from '$lib/ipc/bindings/KnowledgeRemote';
import type { MemoryRemoteKind } from '$lib/ipc/bindings/MemoryRemoteKind';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';

import { RemoteDatabaseStore, type RemoteDatabaseProbe } from './remoteDatabase.svelte';

export interface KnowledgeRemoteSave {
  enabled: boolean;
  kind: MemoryRemoteKind | null;
  /** Null keeps the saved URL. */
  url: string | null;
  /** Null keeps the saved token, and an empty string clears it. */
  token: string | null;
}

export type KnowledgeRemoteProbe = RemoteDatabaseProbe<MemoryRemoteKind>;

/** The team-database setting of Harness Knowledge: the same store as Harness Memory's, over the knowledge commands. */
export class KnowledgeRemoteStore extends RemoteDatabaseStore<KnowledgeRemote, KnowledgeRemoteSave, KnowledgeRemoteProbe, KnowledgeCheck> {
  constructor(transport: Transport) {
    super({
      get: () => transport.invoke('knowledge_remote_get', undefined),
      set: (args) => transport.invoke('knowledge_remote_set', args),
      test: (args) => transport.invoke('knowledge_remote_test', args),
    });
  }
}

export const knowledgeRemote = new KnowledgeRemoteStore(ipc);
