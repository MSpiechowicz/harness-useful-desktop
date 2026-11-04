import { render, screen } from '@testing-library/svelte';

import type { KnowledgeRemote } from '$lib/ipc/bindings/KnowledgeRemote';
import { knowledgeHandlers, type KnowledgeScenario, MockKnowledge } from '$lib/ipc/mock/knowledge-handlers';
import { knowledgeImportHandlers } from '$lib/ipc/mock/knowledge-import-handlers';
import { knowledgeReviewHandlers } from '$lib/ipc/mock/knowledge-review-handlers';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
import { knowledgeHeld } from '$lib/stores/knowledgeHeld.svelte';
import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';
import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
import { knowledgeReview } from '$lib/stores/knowledgeReview.svelte';
import { knowledgeReviewDraft } from '$lib/stores/knowledgeReviewDraft.svelte';
import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
import { knowledgeReplace } from '$lib/stores/knowledgeReplace.svelte';
import { knowledgeStats } from '$lib/stores/knowledgeStats.svelte';
import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
import { projectScopes } from '$lib/stores/project-scopes.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { spyOnInvoke } from '../../../tests/doc-support';
import KnowledgeView from './KnowledgeView.svelte';

/**
 * Who the server says the user is, for a test that needs another person than the preview's: a name, `{ name: null }` for a user with no git name (the stats say
 * so and the pulse stays silent), or `'unknown'` for a server that predates `me` (neither says anything).
 */
export type Viewer = { name: string | null } | 'unknown';

type Handler = (args: unknown) => unknown;

/** Puts who the server says the user is in the answers that carry it. */
function viewedAs(handlers: Record<string, Handler>, viewer: Viewer): Record<string, Handler> {
  const named = viewer === 'unknown' ? null : viewer.name;
  const pulseOf = (pulse: Record<string, unknown>): Record<string, unknown> => {
    const { me: _me, ...rest } = pulse;

    return named === null ? rest : { ...rest, me: named };
  };

  return {
    ...handlers,
    knowledge_stats: async (args) => {
      const { me: _me, ...rest } = (await handlers.knowledge_stats(args)) as Record<string, unknown>;

      return viewer === 'unknown' ? rest : { ...rest, me: viewer.name === null ? {} : { name: viewer.name } };
    },
    knowledge_changes_listen: (args) =>
      handlers.knowledge_changes_listen({
        ...(args as { onEvent: (pulse: Record<string, unknown>) => void }),
        onEvent: (pulse: Record<string, unknown>) => (args as { onEvent: (pulse: unknown) => void }).onEvent(pulseOf(pulse)),
      }),
  };
}

/** A Harness Knowledge that keeps its documents in memory, as the preview does, and the spy that records what the view asked of it. `offline`: its remote database cannot be reached. `viewer`: who it says the user is. */
export function fakeKnowledge(scenario: KnowledgeScenario = 'ok', split = false, extra: Record<string, (args: unknown) => unknown> = {}, offline = false, viewer?: Viewer) {
  const store = new MockKnowledge(scenario, split, offline);
  const handlers = { ...knowledgeHandlers(store, () => scenario, () => true), ...knowledgeImportHandlers(store), ...knowledgeReviewHandlers(store, () => scenario) } as unknown as Record<string, (args: unknown) => unknown>;
  // The real server answers with what it has now, not with the objects it keeps: a read after a change must not find the old answer changed under it.
  const answering = Object.fromEntries(Object.entries(handlers).map(([name, handler]) => [name, async (args: unknown) => structuredClone(await handler(args))]));
  const invoke = spyOnInvoke({ ...(viewer ? viewedAs(answering, viewer) : answering), ...extra });

  return { store, invoke };
}

/** Renders the Knowledge view and waits for its table. */
export async function show(): Promise<void> {
  render(KnowledgeView);
  await screen.findByRole('grid', { name: 'Documents' });
}

/** Forgets what the stores of the Knowledge view read, so each test starts from nothing. */
export function resetKnowledge(): void {
  viewState.reset('knowledge');
  knowledgeDocs.list.sync(null, null);
  knowledgeDocs.detail.sync(null, null);
  knowledgeDocs.repos.sync(null, null);
  knowledgeHeld.list.sync(null, null);
  knowledgeHeld.detail.sync(null, null);
  knowledgeImport.reset();
  knowledgeRemote.value = null;
  knowledgeReplace.reset();
  knowledgeStats.reset();
  knowledgeStatus.value = null;
  knowledgeChanges.reset();
  knowledgeReview.reset();
  knowledgeReviewDraft.reset();
  knowledgeViewer.reset();
  projectScopes.sync(null, null);
}

/** The settings a server with a remote database reports, with the routing of documents given. */
export function routedRemote(repositoryDocuments: KnowledgeRemote['repositoryDocuments'], globalDocuments: KnowledgeRemote['globalDocuments']): Record<string, (args: unknown) => unknown> {
  const remote: KnowledgeRemote = { enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/knowledge', hasToken: false, embedderSupported: true, repositoryDocuments, globalDocuments, repositoryDocumentsEnv: false, globalDocumentsEnv: false, routingSupported: true, configPath: '', problem: null };

  return { knowledge_remote_get: () => remote };
}
