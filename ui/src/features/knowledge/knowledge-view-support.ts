import { render, screen } from '@testing-library/svelte';

import type { KnowledgeRemote } from '$lib/ipc/bindings/KnowledgeRemote';
import { knowledgeHandlers, type KnowledgeScenario, MockKnowledge } from '$lib/ipc/mock/knowledge-handlers';
import { knowledgeImportHandlers } from '$lib/ipc/mock/knowledge-import-handlers';
import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
import { knowledgeHeld } from '$lib/stores/knowledgeHeld.svelte';
import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';
import { knowledgeRemote } from '$lib/stores/knowledgeRemote.svelte';
import { knowledgeReplace } from '$lib/stores/knowledgeReplace.svelte';
import { knowledgeStats } from '$lib/stores/knowledgeStats.svelte';
import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
import { projectScopes } from '$lib/stores/project-scopes.svelte';
import { viewState } from '$lib/stores/view-state.svelte';

import { spyOnInvoke } from '../../../tests/doc-support';
import KnowledgeView from './KnowledgeView.svelte';

/** A Harness Knowledge that keeps its documents in memory, as the preview does, and the spy that records what the view asked of it. `offline`: its remote database cannot be reached. */
export function fakeKnowledge(scenario: KnowledgeScenario = 'ok', split = false, extra: Record<string, (args: unknown) => unknown> = {}, offline = false) {
  const store = new MockKnowledge(scenario, split, offline);
  const handlers = { ...knowledgeHandlers(store, () => scenario, () => true), ...knowledgeImportHandlers(store) } as unknown as Record<string, (args: unknown) => unknown>;
  // The real server answers with what it has now, not with the objects it keeps: a read after a change must not find the old answer changed under it.
  const answering = Object.fromEntries(Object.entries(handlers).map(([name, handler]) => [name, async (args: unknown) => structuredClone(await handler(args))]));
  const invoke = spyOnInvoke({ ...answering, ...extra });

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
  projectScopes.sync(null, null);
}

/** The settings a server with a remote database reports, with the routing of documents given. */
export function routedRemote(repositoryDocuments: KnowledgeRemote['repositoryDocuments'], globalDocuments: KnowledgeRemote['globalDocuments']): Record<string, (args: unknown) => unknown> {
  const remote: KnowledgeRemote = { enabled: true, kind: 'postgres', urlRedacted: 'postgres://db/knowledge', hasToken: false, embedderSupported: true, repositoryDocuments, globalDocuments, repositoryDocumentsEnv: false, globalDocumentsEnv: false, routingSupported: true, configPath: '', problem: null };

  return { knowledge_remote_get: () => remote };
}
