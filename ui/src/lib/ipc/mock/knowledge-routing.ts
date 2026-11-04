import type { DocumentRouting } from '$lib/utils/knowledge-routing';

import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeId } from '../bindings/KnowledgeId';

/** Starting routing of documents, picked with `?kroute=` in the preview: `remote` (the default), `local`, `remote+local` or `split` (project documents remote, global ones on this computer). */
export const KNOWLEDGE_ROUTES = ['remote', 'local', 'remote+local', 'split'] as const;
export type KnowledgeRoute = (typeof KNOWLEDGE_ROUTES)[number];

/** `?koffline=1` pretends the remote database cannot be reached. It needs both places, so it starts routed to both unless `?kroute=` says otherwise. */
export function knowledgeOfflineFromLocation(search: string = globalThis.location?.search ?? ''): boolean {
  return new URLSearchParams(search).get('koffline') === '1';
}

export function knowledgeRoutingFromLocation(search: string = globalThis.location?.search ?? ''): DocumentRouting {
  const name = new URLSearchParams(search).get('kroute') ?? (knowledgeOfflineFromLocation(search) ? 'remote+local' : null);

  switch (KNOWLEDGE_ROUTES.find((route) => route === name)) {
    case 'local':
      return { repositoryDocuments: 'local', globalDocuments: 'local' };
    case 'remote+local':
      return { repositoryDocuments: 'remoteLocal', globalDocuments: 'remoteLocal' };
    case 'split':
      return { repositoryDocuments: 'remote', globalDocuments: 'local' };
    default:
      return { repositoryDocuments: 'remote', globalDocuments: 'remote' };
  }
}

/** The number inside an id: `local:12` and 12 are both 12. */
export function numberOf(id: KnowledgeId): number {
  return Number(String(id).replace(/^local:/, '').split(':')[0]);
}

/** A copy of `doc` kept in the other place, as the server shows it when both places are in use: local ids are `local:N`. */
export function placed(doc: KnowledgeDocument, local: boolean): KnowledgeDocument {
  const where = (id: KnowledgeId): KnowledgeId => (local ? `local:${numberOf(id)}` : numberOf(id));

  return { ...doc, id: where(doc.id), pendingOf: doc.pendingOf == null ? null : where(doc.pendingOf), place: local ? 'local' : 'remote' };
}

/** Whether a document kept in `place` is in the team database: where it says so, else where the routing sends project documents (remote unless they are kept on this computer). */
export function keptInTeam(place: 'remote' | 'local' | undefined, routing: DocumentRouting = knowledgeRoutingFromLocation()): boolean {
  return place === undefined ? routing.repositoryDocuments !== 'local' : place === 'remote';
}

/**
 * The server's refusal of a team document linked to a folder without a git remote (only a `git:` project key leaves this computer): its sentence names the
 * folder, not the path. Null when no link is such a folder.
 */
export function localPathMessage(links: readonly { repo: string }[]): string | null {
  const folder = links.find((link) => link.repo.startsWith('path:'))?.repo.split(/[\\/]/).filter(Boolean).at(-1);

  return folder === undefined ? null : `${folder} has no git remote, so a team document cannot be linked to it (only a git: project key leaves this computer)`;
}
