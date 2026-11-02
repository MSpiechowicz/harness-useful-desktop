import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import type { DocumentRouting } from '$lib/utils/knowledge-routing';

/**
 * Whether a document is kept in the remote database. A document says where it is kept only while both places are in use. Otherwise it is where the routing
 * sends documents: remote unless every kind is kept on this computer, and remote while the routing was not read yet.
 */
export function keptRemote(doc: Pick<KnowledgeDocument, 'place'>, routing: DocumentRouting | null): boolean {
  if (doc.place !== undefined) {
    return doc.place === 'remote';
  }

  return routing === null || routing.repositoryDocuments !== 'local' || routing.globalDocuments !== 'local';
}
