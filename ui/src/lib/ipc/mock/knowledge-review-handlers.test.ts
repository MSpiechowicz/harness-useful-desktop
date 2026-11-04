import { describe, expect, it } from 'vitest';

import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeImportEvent } from '../bindings/KnowledgeImportEvent';
import type { KnowledgeListFilter } from '../bindings/KnowledgeListFilter';
import type { KnowledgeOutcome } from '../bindings/KnowledgeOutcome';
import type { KnowledgePage } from '../bindings/KnowledgePage';
import type { KnowledgePulse } from '../bindings/KnowledgePulse';
import type { KnowledgeReviewSubmit } from '../bindings/KnowledgeReviewSubmit';
import type { KnowledgeStats } from '../bindings/KnowledgeStats';
import type { KnowledgeStatus } from '../bindings/KnowledgeStatus';
import { knowledgeHandlers, MockKnowledge, type KnowledgeScenario } from './knowledge-handlers';
import { knowledgeImportHandlers } from './knowledge-import-handlers';
import { knowledgeReviewHandlers } from './knowledge-review-handlers';

function backend(scenario: KnowledgeScenario = 'drafts') {
  const store = new MockKnowledge(scenario);

  return { store, ...knowledgeHandlers(store, () => scenario, () => true), ...knowledgeImportHandlers(store), ...knowledgeReviewHandlers(store, () => scenario) };
}

const FILTER: KnowledgeListFilter = { scope: null, format: null, place: null, sort: 'title', order: null };

function listed(handlers: ReturnType<typeof backend>, status: KnowledgeListFilter['status'] = null): KnowledgeDocument[] {
  return (handlers.knowledge_docs_list({ filter: { ...FILTER, status }, offset: 0, limit: 50, query: null }) as KnowledgePage).items;
}

function titles(documents: KnowledgeDocument[]): string[] {
  return documents.map((doc) => doc.title);
}

function refusal(outcome: unknown): string {
  const refused = outcome as Extract<KnowledgeOutcome<unknown>, { type: 'refused' }>;

  return refused.type === 'refused' ? refused.code : refused.type;
}

describe('the drafts preview backend', () => {
  it('lists documents in every state, keeps a pending version out of the list and filters by the state it waits in', () => {
    const handlers = backend();
    const all = listed(handlers);

    expect(all).toHaveLength(15);
    expect(new Set(all.map((doc) => doc.status))).toEqual(new Set(['live', 'draft', 'waiting', 'changesRequested']));
    expect(all.every((doc) => doc.pendingOf == null)).toBe(true);
    expect(all.find((doc) => doc.title === 'Billing API')?.pending).toMatchObject({ id: 15, status: 'waiting', revision: 6 });

    // A live document counts for the state of its pending version too.
    expect(titles(listed(handlers, 'waiting'))).toEqual(['Billing API', 'Data retention policy', 'Install variants', 'Support escalation guide']);
    expect(titles(listed(handlers, 'draft'))).toEqual(['Release process', 'Rollback runbook']);
    expect(titles(listed(handlers, 'changesRequested'))).toEqual(['Onboarding checklist']);
    expect(titles(listed(handlers, 'pending'))).toEqual(['Billing API', 'Release process']);
    expect(listed(handlers, 'live')).toHaveLength(10);
  });

  it('counts drafts, waiting documents and pending versions in the stats and the pulse', () => {
    const handlers = backend();
    const pulses: KnowledgePulse[] = [];
    handlers.knowledge_changes_listen({ onEvent: (pulse) => pulses.push(pulse) });

    expect(handlers.knowledge_stats(undefined)).toMatchObject({ documents: 15, drafts: 2, waiting: 4, waitingForYou: 3, pending: 2, approval: true });
    expect(pulses[0]).toMatchObject({ documents: 15, waiting: 3, waitingAll: 4, approval: true, restartHarnesses: null });

    const older = backend('older');
    expect((older.knowledge_status(undefined) as KnowledgeStatus).features).toMatchObject({ docsDrafts: false, docsApproval: false });
    expect(older.knowledge_stats(undefined)).not.toHaveProperty('waiting');
    expect(older.store.pulse()).not.toHaveProperty('waiting');
    expect(older.store.pulse()).not.toHaveProperty('waitingAll');
    expect(() => older.knowledge_approval_get(undefined)).toThrow();
    expect(() => older.knowledge_doc_history({ id: 1 })).toThrow();
  });

  it('shows the restart notice once, until it is dismissed', () => {
    const handlers = backend('upgraded');

    expect(handlers.store.pulse()).toMatchObject({ restartHarnesses: true });
    handlers.knowledge_restart_notice_dismiss(undefined);
    expect(handlers.store.pulse().restartHarnesses).toBeNull();
  });

  it('moves a document one step and refuses with the codes the server uses', () => {
    const handlers = backend();
    const move = (id: number, action: 'ask' | 'requestChanges' | 'approve' | 'goLive', revision: number, comment: string | null = null) => handlers.knowledge_doc_transition({ id, action, revision, comment });

    expect(refusal(move(12, 'ask', 1))).toBe('changed');
    expect(refusal(move(12, 'goLive', 2))).toBe('approvalOn');
    expect(refusal(move(12, 'approve', 2))).toBe('wrongState');
    expect(refusal(move(13, 'approve', 3))).toBe('selfApproval');
    expect(refusal(move(11, 'requestChanges', 4))).toBe('commentRequired');
    expect(refusal(move(99, 'ask', 1))).toBe('notFound');

    expect(move(12, 'ask', 2)).toEqual({ type: 'done', value: { id: 12, status: 'waiting', revision: 3, promoted: false } });
    expect(move(11, 'requestChanges', 4, 'Cite the source.')).toMatchObject({ type: 'done', value: { status: 'changesRequested', revision: 5 } });
    expect(listed(handlers, 'changesRequested').find((doc) => doc.title === 'Data retention policy')?.lastChange).toMatchObject({ by: 'Ada Lovelace', comment: 'Cite the source.' });
    expect(move(14, 'ask', 6)).toMatchObject({ type: 'done', value: { status: 'waiting' } });
  });

  it('lets only the author take an ask back, whether approval is on or off', () => {
    const handlers = backend();
    const back = (id: number, revision: number) => handlers.knowledge_doc_transition({ id, action: 'takeBack', revision, comment: null });

    expect(refusal(back(13, 2))).toBe('changed');
    expect(refusal(back(12, 2))).toBe('wrongState');
    expect(refusal(back(11, 4))).toBe('notAuthor');
    expect(refusal(back(99, 1))).toBe('notFound');

    const generation = handlers.store.generation;

    expect(back(13, 3)).toEqual({ type: 'done', value: { id: 13, status: 'draft', revision: 4, promoted: false } });
    expect(handlers.store.generation).toBeGreaterThan(generation);
    expect(listed(handlers, 'draft').map((doc) => doc.id)).toContain(13);
    expect((handlers.knowledge_doc_history({ id: 13 }) as { events: { kind: string }[] }).events[0].kind).toBe('withdrawn');

    // With approval off a row left waiting can still be taken back by its author.
    const solo = backend('solo');
    solo.store.review.approval = false;
    expect(solo.knowledge_doc_transition({ id: 13, action: 'takeBack', revision: 3, comment: null })).toMatchObject({ type: 'done', value: { status: 'draft' } });
  });

  it('names the authors of a row that is not live and of a pending version, and a rename makes the renamer one', () => {
    const handlers = backend();
    const authorsOf = (id: number) => (handlers.knowledge_doc_get({ id, chunkId: null }) as { document: KnowledgeDocument }).document;

    // Written by Ada, renamed by Grace, asked by Ada: each once. A live document has none, its pending version has its own.
    expect(authorsOf(13).authors).toEqual(['Ada Lovelace', 'Grace Hopper']);
    expect(authorsOf(11).authors).toEqual(['Grace Hopper']);
    expect(authorsOf(1).authors).toBeNull();
    expect(authorsOf(2).pending?.authors).toEqual(['Grace Hopper']);

    // Ada renames a row Grace wrote: she is an author now and cannot approve it.
    handlers.knowledge_doc_rename({ id: 11, title: 'Data retention rules' });

    expect(authorsOf(11).authors).toEqual(['Grace Hopper', 'Ada Lovelace']);
    expect(refusal(handlers.knowledge_doc_transition({ id: 11, action: 'approve', revision: 4, comment: null }))).toBe('selfApproval');
  });

  it('tells the viewer who they are in the stats and the pulse', () => {
    const handlers = backend();

    expect(handlers.knowledge_stats(undefined)).toMatchObject({ me: { name: 'Ada Lovelace' } });
    expect(handlers.store.pulse()).toMatchObject({ me: 'Ada Lovelace' });

    const older = backend('older');

    expect((older.knowledge_stats(undefined) as KnowledgeStats).me).toBeUndefined();
    expect(older.store.pulse().me).toBeUndefined();
  });

  it('makes a draft live when approval is off, and refuses to ask or approve', () => {
    const handlers = backend('solo');
    const move = (action: 'ask' | 'approve' | 'goLive', revision: number) => handlers.knowledge_doc_transition({ id: 12, action, revision, comment: null });

    expect(refusal(move('ask', 2))).toBe('approvalOff');
    expect(refusal(move('approve', 2))).toBe('approvalOff');
    expect(move('goLive', 2)).toMatchObject({ type: 'done', value: { id: 12, status: 'live', promoted: false } });
    expect(listed(handlers, 'live')).toHaveLength(11);
  });

  it('refuses every change on a team database the user may not change', () => {
    const handlers = backend('readonly');
    const seeded = new MockKnowledge('drafts');
    const reading = knowledgeReviewHandlers(seeded, () => 'readonly');

    expect(refusal(reading.knowledge_doc_transition({ id: 12, action: 'ask', revision: 2, comment: null }))).toBe('readOnly');
    expect(refusal(reading.knowledge_approval_set({ on: false }))).toBe('readOnly');
    expect(handlers.knowledge_approval_get(undefined)).toMatchObject({ writable: false });
  });

  it('promotes a pending version into its live document and moves its history there', () => {
    const handlers = backend();
    const before = (handlers.knowledge_doc_history({ id: 2 }) as { total: number }).total;

    const outcome = handlers.knowledge_doc_transition({ id: 15, action: 'approve', revision: 6, comment: null });

    expect(outcome).toMatchObject({ type: 'done', value: { id: 2, status: 'live', promoted: true } });
    expect(handlers.store.pendingOf(handlers.store.find(2))).toBeUndefined();
    expect(() => handlers.store.find(15)).toThrow();
    expect(listed(handlers).find((doc) => doc.title === 'Billing API')).toMatchObject({ id: 2, status: 'live', pending: null, revision: 1, updatedBy: { name: 'Grace Hopper' } });
    expect((handlers.knowledge_doc_history({ id: 2 }) as { events: { kind: string }[]; total: number }).total).toBe(before + 6);
    expect((handlers.knowledge_doc_history({ id: 2 }) as { events: { kind: string }[] }).events[0]).toMatchObject({ kind: 'approved', by: 'Ada Lovelace' });
  });

  it('compares a pending version with its live document, a new document with nothing, and an asked version by its revision', () => {
    const handlers = backend();
    const compare = (id: number, since: number | null = null) => handlers.knowledge_doc_compare({ id, since }) as Extract<KnowledgeOutcome<{ sections: { path: string; status: string; additions: number; hunks: { lines: { kind: string }[] }[] }[]; live: unknown; revision: number }>, { type: 'done' }>;

    const update = compare(15);
    expect(update.value.live).toBe(2);
    expect(update.value.sections.map((section) => [section.path, section.status])).toEqual([['Install', 'modified'], ['Configuration', 'modified'], ['Rollback', 'added'], ['Troubleshooting', 'deleted']]);
    expect(update.value.sections[0].hunks[0].lines.map((line) => line.kind)).toEqual(['context', 'remove', 'add', 'add', 'context']);

    const fresh = compare(12);
    expect(fresh.value.live).toBeNull();
    expect(fresh.value.sections.every((section) => section.status === 'added' && section.additions > 0)).toBe(true);

    expect(compare(15, 5).value.sections.map((section) => section.path)).toEqual(['Install']);

    // The revision of the row that is reviewed, whichever row was asked for.
    expect(update.value.revision).toBe(6);
    expect(compare(2).value.revision).toBe(6);
    expect(fresh.value.revision).toBe(2);
    expect(refusal(handlers.knowledge_doc_compare({ id: 15, since: 2 }))).toBe('sinceUnavailable');
    expect(refusal(handlers.knowledge_doc_compare({ id: 1, since: null }))).toBe('notFound');
  });

  it('keeps the comments of a review with their lines, flags outdated ones and takes replies and resolves', () => {
    const handlers = backend();
    const read = () => handlers.knowledge_doc_comments({ id: 2 }) as { id: number; outdated: boolean; resolvedAt: string | null; parentId: number | null; body: string; section: string | null }[];

    expect(read().find((comment) => comment.id === 4)?.outdated).toBe(true);
    expect(read().find((comment) => comment.id === 5)?.resolvedAt).not.toBeNull();

    const review: KnowledgeReviewSubmit = { verdict: 'comment', body: 'One more thing.', comments: [{ section: 'Install', side: 'new', from: 1, to: 2, body: ' Pin the version. ' }] };
    expect(refusal(handlers.knowledge_doc_review({ id: 15, revision: 5, review }))).toBe('changed');
    expect(handlers.knowledge_doc_review({ id: 15, revision: 6, review })).toMatchObject({ type: 'done', value: { status: 'waiting', promoted: false } });
    expect(read().slice(-2).map((comment) => [comment.section, comment.body])).toEqual([[null, 'One more thing.'], ['Install', 'Pin the version.']]);

    expect(handlers.knowledge_doc_comment_reply({ id: 15, commentId: 3, body: 'Done.' })).toMatchObject({ type: 'done', value: { comment: { parentId: 2, body: 'Done.' } } });
    expect(handlers.knowledge_doc_comment_resolve({ id: 15, commentId: 2, resolved: true })).toMatchObject({ type: 'done', value: { comment: { resolvedBy: 'Ada Lovelace' } } });
    expect(handlers.knowledge_doc_comment_resolve({ id: 15, commentId: 2, resolved: false })).toMatchObject({ type: 'done', value: { comment: { resolvedAt: null } } });
    expect(refusal(handlers.knowledge_doc_comment_reply({ id: 15, commentId: 999, body: 'x' }))).toBe('notFound');
  });

  it('deletes only the signed-in person\'s comments: a root with its replies, a reply alone, never somebody else\'s', () => {
    const handlers = backend();
    const read = () => (handlers.knowledge_doc_comments({ id: 2 }) as { id: number }[]).map((comment) => comment.id);

    expect(refusal(handlers.knowledge_doc_comment_delete({ id: 15, commentId: 3 }))).toBe('notAuthor');
    expect(handlers.knowledge_doc_comment_reply({ id: 15, commentId: 2, body: 'My answer.' })).toMatchObject({ type: 'done' });
    expect(read()).toContain(3);

    // The answer of Grace (3) and the one just written go with Ada's question (2). The others stay.
    expect(handlers.knowledge_doc_comment_delete({ id: 15, commentId: 2 })).toMatchObject({ type: 'done', value: { deleted: [2, 3, 101] } });
    expect(read()).toEqual([1, 4, 5, 6]);
    expect(refusal(handlers.knowledge_doc_comment_delete({ id: 15, commentId: 2 }))).toBe('notFound');
    expect(refusal(backend('readonly').knowledge_doc_comment_delete({ id: 15, commentId: 2 }))).toBe('readOnly');
  });

  it('lets a reviewer approve or ask for changes with a review, and not the author', () => {
    const handlers = backend();
    const review = (verdict: 'approve' | 'requestChanges', body: string | null) => ({ verdict, body, comments: [] });

    expect(refusal(handlers.knowledge_doc_review({ id: 13, revision: 3, review: review('approve', null) }))).toBe('selfApproval');
    expect(refusal(handlers.knowledge_doc_review({ id: 11, revision: 4, review: review('requestChanges', null) }))).toBe('commentRequired');
    expect(handlers.knowledge_doc_review({ id: 11, revision: 4, review: review('requestChanges', 'Cite the source.') })).toMatchObject({ type: 'done', value: { status: 'changesRequested' } });
    expect((handlers.knowledge_doc_comments({ id: 11 }) as { body: string }[]).map((comment) => comment.body)).toEqual(['Cite the source.']);
  });

  it('numbers the sections that share a heading, so a repeated heading and one named like its counter stay apart', () => {
    const handlers = backend();
    const compare = handlers.knowledge_doc_compare({ id: 17, since: null }) as Extract<KnowledgeOutcome<{ sections: { path: string; occurrence: number }[] }>, { type: 'done' }>;

    expect(compare.value.sections.map((section) => [section.path, section.occurrence])).toEqual([['Install', 1], ['Install', 2], ['Install#2', 1], ['Reference', 1]]);

    // The comment keeps the occurrence it was written on (the first when the page names none).
    const review: KnowledgeReviewSubmit = { verdict: 'comment', body: null, comments: [{ section: 'Install', occurrence: 2, side: 'new', from: 1, to: 1, body: 'Second one' }, { section: 'Install', side: 'new', from: 1, to: 1, body: 'First one' }] };
    handlers.knowledge_doc_review({ id: 17, revision: 2, review });

    const stored = handlers.knowledge_doc_comments({ id: 17 }) as { body: string; occurrence: number | null }[];
    expect(stored.map((comment) => [comment.body, comment.occurrence])).toEqual([['Second one', 2], ['First one', 1]]);
  });

  it('reveals a saved download and nothing outside the downloads', () => {
    const handlers = backend();

    expect(handlers.knowledge_reveal_download({ path: '/home/demo/Downloads/architecture.md' })).toBeNull();

    for (const path of ['/etc/passwd', '/home/demo/Downloads/../.ssh/id_ed25519', 'architecture.md']) {
      expect(() => handlers.knowledge_reveal_download({ path })).toThrow();
    }
  });

  it('writes the original or the text where the user chose', () => {
    const handlers = backend();

    expect(handlers.knowledge_doc_download({ id: 1, name: 'architecture.md', live: null })).toEqual({ type: 'done', value: { path: '/home/demo/Downloads/architecture.md', original: true } });
    expect(refusal(handlers.knowledge_doc_download({ id: 99, name: null, live: null }))).toBe('notFound');
  });

  it('switches team approval at once and moves the generation', () => {
    const handlers = backend('solo');
    const generation = handlers.store.generation;

    expect(handlers.knowledge_approval_get(undefined)).toEqual({ approval: false, place: 'remote', writable: true, changedBy: null, changedAt: null });

    const set = handlers.knowledge_approval_set({ on: true });
    const changedAt = handlers.store.review.changedAt;

    expect(set).toEqual({ type: 'done', value: { approval: true, place: 'remote', writable: true, changedBy: 'Ada Lovelace', changedAt } });
    expect(Number.isNaN(Date.parse(changedAt ?? ''))).toBe(false);
    expect(handlers.knowledge_approval_get(undefined)).toMatchObject({ approval: true, changedBy: 'Ada Lovelace', changedAt });
    expect(handlers.store.generation).toBeGreaterThan(generation);
  });

  it('stores a draft for an import and a pending version for a replacement while approval is on', async () => {
    const handlers = backend();
    const events: KnowledgeImportEvent[] = [];
    const before = listed(handlers).length;

    const result = await handlers.knowledge_import({
      token: 'intake-2-0',
      options: { links: [], replaces: null, replacing: [{ path: '/home/demo/Documents/billing-openapi-v2.yaml', id: 2 }, { path: '/home/demo/Documents/architecture.md', id: 999 }], ask: true, note: 'New retry rules' },
      onEvent: (event) => events.push(event),
    });

    const files = events.filter((event) => event.type === 'file');
    expect(files.map((event) => [event.status, event.draft, event.asked])).toEqual([['added', true, true], ['pending', true, true]]);
    expect(result.counts).toMatchObject({ added: 1, pending: 1 });
    expect(listed(handlers).length).toBe(before + 1);
    expect(listed(handlers).find((doc) => doc.id === 2)?.pending).toMatchObject({ status: 'waiting' });
  });

  it('refuses to ask for approval while approval is off, and imports live', async () => {
    const handlers = backend('ok');

    await expect(handlers.knowledge_import({ token: 'intake-1-0', options: { links: [], replaces: 1, replacing: [], ask: true }, onEvent: () => {} })).rejects.toMatchObject({ code: 'invalidInput' });

    const events: KnowledgeImportEvent[] = [];
    await handlers.knowledge_import({ token: 'intake-1-0', options: null, onEvent: (event) => events.push(event) });
    expect(events.find((event) => event.type === 'file')).toMatchObject({ status: 'added', draft: false, asked: false });
  });

  it('approves a proposal as a draft or a pending version while approval is on, and keeps who it is for', () => {
    const handlers = backend();
    const queue = handlers.knowledge_held_list({ offset: 0, limit: 50, query: null, scope: null, sort: null, order: null }) as { items: { id: string; replaces: number | null; revision: string; addedBy?: string | null; status?: string }[] };

    expect(queue.items.map((item) => item.status)).toContain('changesRequested');
    expect(queue.items.every((item) => item.addedBy === 'Ada Lovelace')).toBe(true);

    const replacement = queue.items.find((item) => item.replaces === 2);
    const added = queue.items.find((item) => item.replaces === null && item.status === 'waiting');
    expect(handlers.knowledge_held_decide({ id: replacement?.id ?? '', decision: 'approve', revision: replacement?.revision ?? null })).toMatchObject({ type: 'approved', status: 'pending', draft: true });
    expect(handlers.knowledge_held_decide({ id: added?.id ?? '', decision: 'approve', revision: added?.revision ?? null })).toMatchObject({ type: 'approved', status: 'added', draft: true });
    expect(listed(handlers, 'draft').map((doc) => doc.title)).toContain('Install notes for Windows');
  });

  it('sorts the queue by the person a proposal is written for with unknown names last', () => {
    const handlers = backend();
    const names = ['bob', null, 'Cara', ' Ada Lovelace '];
    handlers.store.held.forEach((item, index) => {
      item.addedBy = names[index % names.length];
    });
    const sorted = (order: 'asc' | 'desc') => (handlers.knowledge_held_list({ offset: 0, limit: 50, query: null, scope: null, sort: 'addedBy', order }) as { items: { addedBy?: string | null }[] }).items.map((item) => item.addedBy?.trim().toLowerCase() ?? null);
    const known = (list: (string | null)[]) => list.filter((name): name is string => name !== null);

    for (const order of ['asc', 'desc'] as const) {
      const list = sorted(order);
      const unknown = list.length - known(list).length;

      expect(unknown).toBeGreaterThan(0);
      expect(list.slice(list.length - unknown).every((name) => name === null)).toBe(true);
      expect(known(list)).toEqual([...known(list)].sort((left, right) => left.localeCompare(right) * (order === 'asc' ? 1 : -1)));
    }
  });

  it('asks the agent of a proposal for changes and says whether its chat is open', () => {
    const handlers = backend();
    const queue = handlers.knowledge_held_list({ offset: 0, limit: 50, query: null, scope: null, sort: 'title', order: null }) as { items: { id: string; session: string | null; revision: string }[] };
    const open = queue.items.find((item) => item.session === 'a1b2c3');
    const closed = queue.items.find((item) => item.session === 'gone-chat');

    const project = '/home/dev/projects/harness-desktop';

    expect(handlers.knowledge_held_follow_up_available({ session: 'a1b2c3', projectPath: project })).toBe(true);
    expect(handlers.knowledge_held_follow_up_available({ session: 'a1b2c3', projectPath: '/home/dev/projects/other' })).toBe(false);
    expect(handlers.knowledge_held_follow_up_available({ session: 'a1b2c3', projectPath: null })).toBe(false);
    expect(handlers.knowledge_held_follow_up_available({ session: 'gone-chat', projectPath: project })).toBe(false);
    expect(handlers.knowledge_held_follow_up_available({ session: null, projectPath: project })).toBe(false);

    expect(handlers.knowledge_held_request_changes({ id: open?.id ?? '', revision: open?.revision ?? null, comment: ' Add the table ' })).toEqual({ decided: { type: 'changesRequested' }, delivered: true });
    expect(handlers.knowledge_held_request_changes({ id: closed?.id ?? '', revision: null, comment: 'Shorter.' })).toEqual({ decided: { type: 'changesRequested' }, delivered: false });
    expect(handlers.knowledge_held_request_changes({ id: closed?.id ?? '', revision: null, comment: ' ' })).toMatchObject({ decided: { type: 'commentRequired' }, delivered: false });
    expect(handlers.knowledge_held_request_changes({ id: 'gone', revision: null, comment: 'x' })).toMatchObject({ decided: { type: 'notFound' } });
    expect((handlers.knowledge_stats(undefined) as KnowledgeStats).held).toBeUndefined();
  });

  it('deletes the pending version with its live document, or alone to discard the update', () => {
    const handlers = backend();

    expect(handlers.knowledge_docs_delete({ ids: [15] })).toEqual({ deleted: 0 });
    expect(handlers.store.pendingOf(handlers.store.find(2))).toBeUndefined();

    expect(handlers.knowledge_docs_delete({ ids: [5] })).toEqual({ deleted: 1 });
    expect(handlers.store.documents.some((doc) => doc.id === 16)).toBe(false);
  });
});
