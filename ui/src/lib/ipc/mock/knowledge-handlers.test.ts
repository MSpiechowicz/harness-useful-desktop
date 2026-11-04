import { describe, expect, it } from 'vitest';

import type { KnowledgeDetail } from '../bindings/KnowledgeDetail';
import type { KnowledgeImportEvent } from '../bindings/KnowledgeImportEvent';
import type { KnowledgePulse } from '../bindings/KnowledgePulse';
import type { KnowledgeStatus } from '../bindings/KnowledgeStatus';
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

    const pdf = knowledge_docs_list({ ...ALL, filter: { scope: null, format: 'pdf', place: null, sort: 'title', order: null } }) as typeof first;
    expect(pdf.items.every((doc) => doc.format === 'pdf')).toBe(true);
    expect(pdf.items.map((doc) => doc.title)).toEqual([...pdf.items.map((doc) => doc.title)].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase())).slice(0, 50));

    const found = knowledge_docs_list({ ...ALL, query: 'transcript' }) as typeof first;
    expect(found.items.map((doc) => doc.format)).toEqual(['transcript']);
  });

  it('scopes the list to one repository or to the global documents', () => {
    const { knowledge_docs_list } = backend();
    const scoped = (scope: { kind: 'global' } | { kind: 'repo'; key: string }) => knowledge_docs_list({ ...ALL, filter: { scope, format: null, place: null, sort: null, order: null } }) as Awaited<ReturnType<typeof knowledge_docs_list>>;

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

    const queue = knowledge_held_list({ offset: 0, limit: 50, query: null, scope: null, sort: null, order: null }) as Awaited<ReturnType<typeof knowledge_held_list>>;
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
    expect(full).toEqual({ counts: { added: 3, updated: 0, pending: 0, unchanged: 0, skipped: 1, failed: 1 }, cancelled: false });
  });

  it('keeps documents by whether agents found them useful and sorts by it, as the server does', () => {
    const { knowledge_docs_list } = backend();
    const listed = (filter: Partial<NonNullable<Parameters<typeof knowledge_docs_list>[0]['filter']>>) =>
      (knowledge_docs_list({ ...ALL, filter: { scope: null, format: null, place: null, sort: null, order: null, ...filter } }) as Awaited<ReturnType<typeof knowledge_docs_list>>).items;

    const ever = listed({ useful: 'ever' });
    expect(ever.length).toBeGreaterThan(0);
    expect(ever.every((doc) => (doc.useful ?? 0) > 0)).toBe(true);

    const never = listed({ useful: 'never' });
    expect(never.length).toBeGreaterThan(0);
    expect(never.every((doc) => doc.useful === 0 && (doc.shown ?? 0) >= 3)).toBe(true);
    expect(ever.length + never.length).toBeLessThan(listed({}).length);

    const useful = listed({ sort: 'useful' }).map((doc) => doc.useful ?? 0);
    expect(useful).toEqual([...useful].sort((a, b) => b - a));
    expect(listed({ sort: 'useful', order: 'asc' }).map((doc) => doc.useful ?? 0)).toEqual([...useful].reverse());

    const latest = listed({ sort: 'lastUseful' }).map((doc) => (doc.lastUsefulAt ? Date.parse(doc.lastUsefulAt) : 0));
    expect(latest).toEqual([...latest].sort((a, b) => b - a));
  });

  it('carries who added and changed a document, and says what an older Harness Knowledge does not', () => {
    const current = backend();
    const older = backend('older');
    const first = (current.knowledge_docs_list(ALL) as { items: { id: number | string }[] }).items;
    const rows = (backend_: ReturnType<typeof backend>) => (backend_.knowledge_docs_list({ ...ALL, filter: { scope: null, format: null, place: null, sort: 'useful', order: null, useful: 'ever' } }) as { items: Record<string, unknown>[] }).items;

    expect((current.knowledge_status(undefined) as KnowledgeStatus).features).toMatchObject({ docsUseful: true, docsSimilar: true, docsAuthors: true });
    expect((current.knowledge_doc_get({ id: first[0].id, chunkId: null }) as KnowledgeDetail).document).toMatchObject({ addedBy: { name: 'Ada Lovelace', via: null }, updatedBy: { name: 'Grace Hopper', via: null } });
    expect(rows(current).length).toBeGreaterThan(0);

    expect((older.knowledge_status(undefined) as KnowledgeStatus).features).toMatchObject({ docs: true, docsUseful: false, docsSimilar: false, docsAuthors: false });
    // The filter and the sorts are dropped the way the app drops them, and the rows carry none of the new fields.
    const plain = rows(older);
    expect(plain).toHaveLength(10);
    expect(plain.every((doc) => !('addedBy' in doc) && !('shown' in doc) && !('lastUsefulAt' in doc))).toBe(true);
    expect('addedBy' in (older.knowledge_doc_get({ id: 1, chunkId: null }) as KnowledgeDetail).document).toBe(false);
  });

  it('reports whether the remote place may be changed, unknown while it cannot be reached', () => {
    const writable = (backend_: ReturnType<typeof backend>) => (backend_.knowledge_stats(undefined) as { remoteWritable?: boolean | null }).remoteWritable;
    const unreachable = new MockKnowledge('ok', true, true);

    expect(writable(backend())).toBe(true);
    expect(writable(backend('readonly'))).toBe(false);
    expect(writable(backend('older'))).toBeUndefined();
    expect((knowledgeHandlers(unreachable, () => 'ok', () => true).knowledge_stats(undefined) as { remoteWritable?: boolean | null }).remoteWritable).toBeNull();
  });

  it('finds the stored documents a selection looks like, in the scope of the import, and replaces one in place', async () => {
    const { knowledge_pick_files, knowledge_pick_folder, knowledge_similar, knowledge_import, knowledge_docs_list, store } = backend();
    const intake = knowledge_pick_files(undefined) as { token: string; files: { path: string }[] };
    const found = knowledge_similar({ token: intake.token, links: null }) as Awaited<ReturnType<typeof knowledge_similar>>;
    const byName = (name: string) => found.items.find((entry) => entry.path.endsWith(`/${name}`));

    expect(byName('architecture.md')?.match).toMatchObject({ id: 1, reason: 'sourceKey', title: 'Architecture overview', addedBy: { name: 'Ada Lovelace' }, sharedChunks: 18, chunks: 18 });
    expect(byName('billing-openapi-v2.yaml')?.match).toMatchObject({ id: 2, reason: 'sharedChunks' });
    expect(byName('scan-of-contract.pdf')).toMatchObject({ match: null, error: 'the PDF has no text layer (a scan)' });
    // The title only matches in the scope of the import: the glossary is global.
    expect(byName('glossary.md')?.match).toMatchObject({ id: 10, reason: 'title' });
    expect((knowledge_similar({ token: intake.token, links: [{ repo: 'git:github.com/useful/website', globs: [] }] }) as typeof found).items.find((entry) => entry.path.endsWith('glossary.md'))?.match).toBeNull();

    const folder = knowledge_pick_folder(undefined) as { token: string };
    expect((knowledge_similar({ token: folder.token, links: null }) as typeof found).items.map((entry) => entry.path.split('/').slice(-2).join('/'))).toEqual(['team-docs/handbook.md', 'team-docs/release-process.adoc']);
    expect(() => knowledge_similar({ token: 'intake-forged', links: null })).toThrow();

    const before = store.documents.length;
    const result = await knowledge_import({ token: intake.token, options: { links: [], replaces: null, replacing: [{ path: `${intake.files[0].path}`, id: 1 }] }, onEvent: () => {} });
    const architecture = (knowledge_docs_list({ ...ALL, query: 'Architecture' }) as { items: { id: number | string; addedBy?: { name: string } | null; updatedBy?: { name: string } | null }[] }).items[0];

    expect(result.counts).toMatchObject({ added: 2, updated: 1 });
    expect(store.documents.length).toBe(before + 2);
    expect(architecture).toMatchObject({ id: 1, addedBy: { name: 'Ada Lovelace' }, updatedBy: { name: 'Ada Lovelace' } });
  });

  it('refuses to replace a document that lives in another place than the import files its documents in, as the sidecar does', async () => {
    const store = new MockKnowledge('ok', true);
    const { knowledge_pick_files, knowledge_import } = knowledgeImportHandlers(store);
    const intake = knowledge_pick_files(undefined) as { token: string; files: { path: string }[] };
    const replacing = [{ path: intake.files[0].path, id: 1 }];

    const events: string[] = [];
    await expect(knowledge_import({ token: intake.token, options: { links: [], replaces: null, replacing, place: 'local' }, onEvent: (event) => events.push(event.type) })).rejects.toMatchObject({ code: 'invalidInput' });
    // Refused before the first file, as the sidecar does: nothing was planned or imported.
    expect(events).toEqual([]);

    const result = await knowledge_import({ token: intake.token, options: { links: [], replaces: null, replacing, place: 'remote' }, onEvent: () => {} });
    expect(result.counts?.updated).toBe(1);
  });

  it('replaces a document in its own place when the import names no place, as the sidecar does', async () => {
    const store = new MockKnowledge('ok', true);
    const { knowledge_pick_files, knowledge_import } = knowledgeImportHandlers(store);
    const intake = knowledge_pick_files(undefined) as { token: string; files: { path: string }[] };

    const result = await knowledge_import({ token: intake.token, options: { links: [], replaces: null, replacing: [{ path: intake.files[0].path, id: 1 }] }, onEvent: () => {} });
    expect(result.counts?.updated).toBe(1);
  });
});
