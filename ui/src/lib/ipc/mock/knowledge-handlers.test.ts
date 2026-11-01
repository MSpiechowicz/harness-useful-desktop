import { describe, expect, it } from 'vitest';

import type { KnowledgeImportEvent } from '../bindings/KnowledgeImportEvent';
import type { KnowledgePulse } from '../bindings/KnowledgePulse';
import { knowledgeHandlers, MockKnowledge, type KnowledgeScenario } from './knowledge-handlers';
import { knowledgeImportHandlers } from './knowledge-import-handlers';

function backend(scenario: KnowledgeScenario = 'ok', installed = true) {
  const store = new MockKnowledge(scenario);

  return { store, ...knowledgeHandlers(store, () => scenario, () => installed), ...knowledgeImportHandlers(store) };
}

const ALL = { filter: null, offset: 0, limit: 50, query: null };

describe('the knowledge preview backend', () => {
  it('pages a long list by the server and filters it by format and words', () => {
    const { knowledge_docs_list } = backend('bulk');

    const first = knowledge_docs_list({ ...ALL, limit: 200 }) as Awaited<ReturnType<typeof knowledge_docs_list>>;
    expect(first.total).toBe(1510);
    expect(first.items).toHaveLength(200);

    const last = knowledge_docs_list({ ...ALL, offset: 1500, limit: 50 }) as typeof first;
    expect(last.items).toHaveLength(10);

    const pdf = knowledge_docs_list({ ...ALL, filter: { scope: null, format: 'pdf', sort: 'title', order: null } }) as typeof first;
    expect(pdf.items.every((doc) => doc.format === 'pdf')).toBe(true);
    expect(pdf.items.map((doc) => doc.title)).toEqual([...pdf.items.map((doc) => doc.title)].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase())).slice(0, 50));

    const found = knowledge_docs_list({ ...ALL, query: 'transcript' }) as typeof first;
    expect(found.items.map((doc) => doc.format)).toEqual(['text']);
  });

  it('scopes the list to one repository or to the global documents', () => {
    const { knowledge_docs_list } = backend();
    const scoped = (scope: { kind: 'global' } | { kind: 'repo'; key: string }) => knowledge_docs_list({ ...ALL, filter: { scope, format: null, sort: null, order: null } }) as Awaited<ReturnType<typeof knowledge_docs_list>>;

    expect(scoped({ kind: 'global' }).items.every((doc) => doc.links.length === 0)).toBe(true);
    expect(scoped({ kind: 'repo', key: 'git:github.com/useful/website' }).items.map((doc) => doc.title)).toEqual(['Release process']);
  });

  it('says it is not installed instead of listing anything', () => {
    const { knowledge_docs_list, knowledge_status } = backend('ok', false);

    expect(knowledge_docs_list(ALL)).toMatchObject({ availability: { type: 'notInstalled' }, items: [], total: 0 });
    expect(knowledge_status(undefined)).toMatchObject({ availability: { type: 'notInstalled' }, features: { docs: false, held: false } });
  });

  it('pulses the review count as proposals are decided, and approving one adds its document', () => {
    const { knowledge_changes_listen, knowledge_held_list, knowledge_held_decide, knowledge_docs_list, store } = backend();
    const pulses: KnowledgePulse[] = [];
    knowledge_changes_listen({ onEvent: (pulse) => pulses.push(pulse) });

    const queue = knowledge_held_list({ offset: 0, limit: 50, query: null }) as Awaited<ReturnType<typeof knowledge_held_list>>;
    expect(queue.total).toBe(3);
    expect(pulses[0]).toMatchObject({ held: 3, documents: 10 });

    const [first, second] = queue.items;
    expect(knowledge_held_decide({ id: first.id, decision: 'approve', revision: first.revision })).toMatchObject({ type: 'approved', status: 'added', title: 'Install notes for Windows' });
    expect(knowledge_held_decide({ id: second.id, decision: 'reject', revision: 'stale' })).toMatchObject({ type: 'changed' });
    expect(knowledge_held_decide({ id: second.id, decision: 'reject', revision: second.revision })).toEqual({ type: 'rejected' });
    expect(knowledge_held_decide({ id: second.id, decision: 'reject', revision: null })).toMatchObject({ type: 'notFound' });

    expect(pulses.at(-1)).toMatchObject({ held: 1, documents: 11 });
    expect((knowledge_docs_list({ ...ALL, query: 'Install notes' }) as { total: number }).total).toBe(1);
    expect(store.generation).toBeGreaterThan(pulses[0].generation);
  });

  it('streams an import file by file with its reasons, and keeps what was done before a cancel', async () => {
    const { knowledge_import, knowledge_import_cancel, store } = backend();
    const before = store.documents.length;
    const events: KnowledgeImportEvent[] = [];

    const result = await knowledge_import({
      token: 't',
      options: null,
      onEvent: (event) => {
        events.push(event);

        if (event.type === 'progress' && event.done === 2) {
          knowledge_import_cancel(undefined);
        }
      },
    });

    expect(events[0]).toEqual({ type: 'plan', total: 5 });
    expect(events.filter((event) => event.type === 'file')).toHaveLength(2);
    expect(result.cancelled).toBe(true);
    expect(store.documents.length).toBe(before + 2);
    expect(events.at(-1)).toMatchObject({ type: 'done', cancelled: true });

    // The next import runs to the end, with a failed scan and a skipped file in it.
    const full = await knowledge_import({ token: 't', options: null, onEvent: () => {} });
    expect(full).toEqual({ counts: { added: 3, updated: 0, unchanged: 0, skipped: 1, failed: 1 }, cancelled: false });
  });
});
