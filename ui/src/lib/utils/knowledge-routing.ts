import type { KnowledgePlace } from '$lib/ipc/bindings/KnowledgePlace';
import type { MemoryNotesRouting } from '$lib/ipc/bindings/MemoryNotesRouting';

/** Where project documents and global documents are kept, as the server's `[routing]` says. */
export interface DocumentRouting {
  repositoryDocuments: MemoryNotesRouting;
  globalDocuments: MemoryNotesRouting;
}

/** The routing of a server that routes nothing: everything in the remote database. */
export const REMOTE_ROUTING: DocumentRouting = { repositoryDocuments: 'remote', globalDocuments: 'remote' };

/**
 * Whether both places are in use, which is when documents carry a `place` and local ids. Without a remote database (or on a server
 * without routing) everything is one place. With one, both are used when any kind reads `remote+local`, or the two kinds are split
 * between the places.
 */
export function usesBothPlaces(enabled: boolean, routing: DocumentRouting): boolean {
  if (!enabled) {
    return false;
  }

  const { repositoryDocuments: repo, globalDocuments: global } = routing;

  return repo === 'remoteLocal' || global === 'remoteLocal' || repo !== global;
}

/** The places a kind of document may be saved to under `routing`, and the one it goes to unless chosen. */
export function placesFor(routing: MemoryNotesRouting): { allowed: KnowledgePlace[]; fallback: KnowledgePlace } {
  switch (routing) {
    case 'local':
      return { allowed: ['local'], fallback: 'local' };
    case 'remoteLocal':
      return { allowed: ['remote', 'local'], fallback: 'remote' };
    case 'remote':
      return { allowed: ['remote'], fallback: 'remote' };
  }
}

/** The place an import goes to: the user's choice when the routing allows it for this kind of document, else the routing's default for the kind. */
export function effectivePlace(routing: DocumentRouting, global: boolean, chosen: KnowledgePlace | null): KnowledgePlace {
  const rule = placesFor(global ? routing.globalDocuments : routing.repositoryDocuments);

  return chosen !== null && rule.allowed.includes(chosen) ? chosen : rule.fallback;
}

/** Whether the choice between the places is offered: only when a kind of document reads `remote+local`. */
export function choosesPlace(enabled: boolean, routing: DocumentRouting): boolean {
  return enabled && (routing.repositoryDocuments === 'remoteLocal' || routing.globalDocuments === 'remoteLocal');
}
