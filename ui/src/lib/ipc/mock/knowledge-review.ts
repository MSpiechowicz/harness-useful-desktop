import type { AuthorRef } from '../bindings/AuthorRef';
import type { KnowledgeComment } from '../bindings/KnowledgeComment';
import type { KnowledgeCompare } from '../bindings/KnowledgeCompare';
import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeHeldItem } from '../bindings/KnowledgeHeldItem';
import type { KnowledgeHistoryEvent } from '../bindings/KnowledgeHistoryEvent';
import type { KnowledgeId } from '../bindings/KnowledgeId';
import type { KnowledgePending } from '../bindings/KnowledgePending';
import type { KnowledgeQuoteLine } from '../bindings/KnowledgeQuoteLine';
import { idKey } from '../knowledge-id';
import { ADA, GRACE } from './authors';
import { ago, chunkText, documentOf, held, outlineOf, REPEATED_HEADINGS_NAME } from './knowledge-fixtures';
import type { MockKnowledge } from './knowledge-handlers';
import { diffOf, newDocumentDiffs, numbered, pendingChanges } from './knowledge-review-diff';

/** The person the preview is signed in as: the git `user.name` the server sees. Approving what this person wrote is refused as `selfApproval`. */
export const ME = ADA.name;

/** Whether two git names are the same person, as the server compares them: trimmed, without regard to case. */
export function sameName(left: string | null | undefined, right: string | null | undefined): boolean {
  return left != null && right != null && left.trim().toLowerCase() === right.trim().toLowerCase();
}

/** The scenarios that start with documents in every state, with team approval on (`drafts`) or off (`solo`). */
export function hasDrafts(scenario: string): boolean {
  return scenario === 'drafts' || scenario === 'solo';
}

/** The ids of the seeded rows, after the sample documents. */
const IDS = { retention: 11, runbook: 12, escalation: 13, checklist: 14, billingUpdate: 15, releaseUpdate: 16, variants: 17 } as const;

/** Rows in every state: a draft of this user, two documents that wait (one written by this user), one with changes requested, and updates of live documents. */
export function seedReviewDocuments(): KnowledgeDocument[] {
  const base = { format: 'markdown', chunks: 5, links: [{ repo: 'git:github.com/useful/harness-desktop', globs: [] }] };

  return [
    documentOf({ ...base, id: IDS.runbook, title: 'Rollback runbook', name: 'rollback-runbook.md', kb: 11, age: 5, status: 'draft', revision: 2, addedBy: ADA }),
    documentOf({ ...base, id: IDS.retention, title: 'Data retention policy', name: 'data-retention.md', kb: 14, age: 9, status: 'waiting', revision: 4, addedBy: GRACE }),
    documentOf({ ...base, id: IDS.escalation, title: 'Support escalation guide', name: 'support-escalation.md', kb: 8, age: 12, status: 'waiting', revision: 3, addedBy: ADA }),
    documentOf({
      ...base,
      id: IDS.checklist,
      title: 'Onboarding checklist',
      name: 'onboarding-checklist.md',
      kb: 6,
      age: 30,
      status: 'changesRequested',
      revision: 6,
      addedBy: ADA,
      lastChange: { by: GRACE.name, at: ago(20), comment: 'The accounts step is out of date. Please add the new SSO sign-in.' },
    }),
    documentOf({ ...base, id: IDS.billingUpdate, title: 'Billing API', name: 'billing-openapi.yaml', format: 'openapi', kb: 190, chunks: 66, age: 4, status: 'waiting', revision: 6, pendingOf: 2, addedBy: GRACE, links: [{ repo: 'git:github.com/useful/harness-desktop', globs: ['src/billing/**'] }] }),
    // Its outline repeats a heading (see `REPEATED_HEADINGS`): a compare numbers the sections that share one.
    documentOf({ ...base, id: IDS.variants, title: 'Install variants', name: REPEATED_HEADINGS_NAME, chunks: 4, kb: 7, age: 3, status: 'waiting', revision: 2, addedBy: GRACE }),
    documentOf({ ...base, id: IDS.releaseUpdate, title: 'Release process', name: 'release-process.adoc', format: 'asciidoc', kb: 28, chunks: 13, age: 2, status: 'draft', revision: 2, pendingOf: 5, addedBy: ADA }),
  ];
}

/** The proposal an agent was asked to revise: the review queue shows it as `changesRequested` with the comment. */
export function seedRevisedProposal(): KnowledgeHeldItem {
  return held({
    id: '5d0c7a44-2b61-4f0a-a7d3-6c1e9b8f3a55',
    title: 'Postgres failover notes',
    name: 'postgres-failover.md',
    client: 'claude-code',
    age: 14,
    session: 'a1b2c3',
    status: 'changesRequested',
    comment: 'Say which replica is promoted first.',
    text: '# Postgres failover notes\n\nPromote the standby, then point the app at it.',
  });
}

function event(kind: KnowledgeHistoryEvent['kind'], by: string | null, hours: number, comment: string | null = null): KnowledgeHistoryEvent {
  return { kind, by, at: ago(hours), comment };
}

/** The history of every seeded row, newest first, with the rounds a live document went through before its update. */
function seedEvents(): [number, KnowledgeHistoryEvent[]][] {
  return [
    [IDS.runbook, [event('edited', ADA.name, 20), event('drafted', ADA.name, 120)]],
    [IDS.retention, [event('asked', GRACE.name, 30), event('edited', GRACE.name, 80), event('drafted', GRACE.name, 216)]],
    // Grace renamed it after Ada wrote it: both are its authors.
    [IDS.escalation, [event('asked', ADA.name, 44), event('edited', GRACE.name, 100, 'renamed'), event('drafted', ADA.name, 288)]],
    [IDS.checklist, [event('changesRequested', GRACE.name, 20, 'The accounts step is out of date. Please add the new SSO sign-in.'), event('asked', ADA.name, 46), event('edited', ADA.name, 60), event('drafted', ADA.name, 720)]],
    [IDS.billingUpdate, [event('edited', GRACE.name, 30), event('asked', GRACE.name, 70), event('updateDrafted', GRACE.name, 96), event('wentLive', GRACE.name, 480), event('drafted', GRACE.name, 500)]],
    [IDS.releaseUpdate, [event('updateDrafted', ADA.name, 48)]],
    [IDS.variants, [event('asked', GRACE.name, 40), event('drafted', GRACE.name, 70)]],
  ];
}

function quoted(line: number, kind: KnowledgeQuoteLine['kind'], text: string): KnowledgeQuoteLine {
  return { line, kind, text };
}

function comment(fields: Partial<KnowledgeComment> & Pick<KnowledgeComment, 'id' | 'body' | 'by'>): KnowledgeComment {
  return { reviewId: 1, section: null, occurrence: fields.section ? 1 : null, side: null, from: null, to: null, at: ago(24), parentId: null, resolvedAt: null, resolvedBy: null, revision: 6, outdated: false, quote: null, ...fields };
}

/** The comments on the pending update of Billing API: an overall one, lines of Ada (one answered, one outdated, one resolved) and one of Grace. The ones on lines quote them. */
function seedComments(): KnowledgeComment[] {
  return [
    comment({ id: 1, body: 'The retry rules look right. Two questions on the install steps.', by: ADA.name, at: ago(26) }),
    comment({ id: 2, section: 'Install', side: 'new', from: 3, to: 4, body: 'Why five retries when the contract says three?', by: ADA.name, at: ago(26), quote: [quoted(3, 'added', 'Back off by one hour between tries.'), quoted(4, 'same', 'Restart the app.')] }),
    comment({ id: 3, parentId: 2, reviewId: null, body: 'The contract changed in September, the old number is gone.', by: GRACE.name, at: ago(22) }),
    comment({ id: 4, section: 'Configuration', side: 'old', from: 2, to: 2, body: 'Keep the old default here.', by: ADA.name, at: ago(26), revision: 4, outdated: true, quote: [quoted(2, 'removed', 'Retries default to 3.')] }),
    comment({ id: 5, section: 'Rollback', side: 'new', from: 1, to: 2, body: 'Add who to page.', by: ADA.name, at: ago(25), resolvedAt: ago(21), resolvedBy: GRACE.name, quote: [quoted(1, 'added', 'Switch the feature flag off.'), quoted(2, 'added', 'Page the on-call engineer.')] }),
    comment({ id: 6, section: 'Configuration', side: 'new', from: 2, to: 2, body: 'Say where the override is read from.', by: GRACE.name, at: ago(23), quote: [quoted(2, 'added', 'Set `BILLING_RETRIES` to override the limit.')] }),
  ];
}

/** What the preview keeps of a review: the team approval switch, the history and comments of every document and the revisions at which approval was asked. */
export class MockReview {
  /** Whether every import to the team database is a draft a teammate approves. */
  approval: boolean;
  /** Who last switched it and when: none until somebody did. */
  changedBy: string | null = null;
  changedAt: string | null = null;
  readonly events = new Map<string, KnowledgeHistoryEvent[]>();
  readonly comments = new Map<string, KnowledgeComment[]>();
  /** The revisions each document was asked at: what `compare` can be asked to compare with. */
  readonly asked = new Map<string, number[]>();
  private commentId = 100;

  constructor(scenario: string) {
    this.approval = scenario === 'drafts';

    if (this.approval) {
      this.changedBy = GRACE.name;
      this.changedAt = ago(72);
    }

    if (!hasDrafts(scenario)) {
      return;
    }

    seedEvents().forEach(([id, events]) => this.events.set(idKey(id), events));
    this.comments.set(idKey(IDS.billingUpdate), seedComments());
    this.asked.set(idKey(IDS.billingUpdate), [5]);
  }

  nextCommentId(): number {
    this.commentId += 1;

    return this.commentId;
  }

  record(id: KnowledgeId, kind: KnowledgeHistoryEvent['kind'], by: string | null, comment: string | null = null): void {
    this.events.set(idKey(id), [event(kind, by, 0, comment), ...(this.events.get(idKey(id)) ?? [])]);
  }

  /**
   * The people the server's approval and take-back rules count as authors of `row`: who it says wrote it last, everyone who wrote or renamed it since it was last live
   * and the latest asker, each once (names compared as `sameName` does).
   */
  authorsOf(row: KnowledgeDocument): string[] {
    const names = [(row.updatedBy ?? row.addedBy)?.name];
    let asked = false;

    for (const entry of this.events.get(idKey(row.id)) ?? []) {
      if (entry.kind === 'approved' || entry.kind === 'wentLive') {
        break;
      }

      if (entry.kind === 'asked' && !asked) {
        asked = true;
        names.push(entry.by ?? undefined);
      } else if (entry.kind === 'drafted' || entry.kind === 'edited' || entry.kind === 'updateDrafted') {
        names.push(entry.by ?? undefined);
      }
    }

    return names.filter((name): name is string => name !== undefined && name.trim() !== '').filter((name, index, all) => all.findIndex((other) => sameName(other, name)) === index);
  }

  /** Whether `name` is one of the authors of `row` (see `authorsOf`): who may not approve it and who may take its ask back. */
  isAuthor(row: KnowledgeDocument, name: string): boolean {
    return this.authorsOf(row).some((author) => sameName(author, name));
  }

  /** The rounds of a pending version join its live document's history when it is promoted. */
  mergeHistory(from: KnowledgeId, into: KnowledgeId): void {
    const merged = [...(this.events.get(idKey(from)) ?? []), ...(this.events.get(idKey(into)) ?? [])].sort((left, right) => right.at.localeCompare(left.at));

    this.events.set(idKey(into), merged);
    this.events.delete(idKey(from));
    this.comments.delete(idKey(from));
  }
}

/** The pending summary a live document carries in a list. */
export function pendingSummary(row: KnowledgeDocument, authors: string[] | null = null): KnowledgePending {
  return { id: row.id, status: row.status, revision: row.revision, updatedBy: row.updatedBy ?? null, updatedAt: row.updatedAt, authors };
}

/** The sections of a pending (or draft) row against its live document, or against nothing when it is a new document. `since` compares with the version that was asked. */
export function compareOf(row: KnowledgeDocument, live: KnowledgeDocument | undefined, since: number | null): KnowledgeCompare {
  if (since !== null) {
    return { live: live?.id ?? null, pending: row.id, revision: row.revision, sections: numbered([diffOf('Install', { before: ['Run the installer from the release page.'], removed: ['Set the retry limit to 4.'], added: ['Set the retry limit to 5.'], after: ['Restart the app.'] })]), truncated: false };
  }

  if (!live) {
    return { live: null, pending: row.id, revision: row.revision, sections: newDocumentDiffs(outlineOf(row).map((section) => ({ heading: section.headingPath, lines: chunkText(row, section).split('\n') }))), truncated: false };
  }

  return { live: live.id, pending: row.id, revision: row.revision, sections: pendingChanges(), truncated: false };
}

/** How many of `rows` wait for this person: waiting and not written by them (the mock does not model who asked, only who wrote). */
export function waitingForYou(rows: KnowledgeDocument[]): number {
  return rows.filter((doc) => doc.status === 'waiting' && !sameName(doc.updatedBy?.name ?? doc.addedBy?.name, ME)).length;
}

/** The counts of `stats` for the drafts: drafts and documents waiting for approval (a pending version counts as its own row), live documents with a pending version, and whether approval is on. */
export function reviewCounts(documents: KnowledgeDocument[], kept: KnowledgeDocument[], approval: boolean): { drafts: number; waiting: number; waitingForYou: number; pending: number; approval: boolean } {
  const ids = new Set(kept.map((doc) => idKey(doc.id)));
  const pending = documents.filter((doc) => doc.pendingOf != null && ids.has(idKey(doc.pendingOf)));

  return {
    drafts: [...kept, ...pending].filter((doc) => doc.status === 'draft').length,
    waiting: [...kept, ...pending].filter((doc) => doc.status === 'waiting').length,
    waitingForYou: waitingForYou([...kept, ...pending]),
    pending: pending.length,
    approval,
  };
}

/** Whether an import (or an approved proposal) to `place` is stored as a draft: team approval is on and the documents go to the team database. */
export function draftsApply(store: MockKnowledge, place: 'remote' | 'local' | null | undefined): boolean {
  return store.drafts && store.review.approval && (place ?? 'remote') === 'remote';
}

/** Files a replacement of `live` as its pending version: a new pending row, or the one it has already, changed and back to a draft (or waiting when `ask`). */
export function putPending(store: MockKnowledge, live: KnowledgeDocument, by: AuthorRef, ask: boolean): KnowledgeDocument {
  const status = ask ? 'waiting' : 'draft';
  const existing = store.pendingOf(live);

  if (existing) {
    const next = { ...existing, status, revision: existing.revision + 1, updatedAt: new Date().toISOString(), updatedBy: by } as const;

    store.documents[store.documents.indexOf(existing)] = next;
    store.review.record(next.id, 'edited', by.name);

    if (ask) {
      store.review.record(next.id, 'asked', by.name);
    }

    return next;
  }

  const row = documentOf({ id: store.newId(live.place), title: live.title, name: live.sourceName, format: live.format, kb: Math.max(1, Math.ceil(live.bytes / 1024)), chunks: live.chunkCount, age: 0, links: live.links, status, revision: 1, pendingOf: live.id, addedBy: by });
  const placed = store.split ? { ...row, place: live.place } : row;

  store.documents.push(placed);
  store.review.record(placed.id, 'updateDrafted', by.name);

  if (ask) {
    store.review.record(placed.id, 'asked', by.name);
  }

  return placed;
}
