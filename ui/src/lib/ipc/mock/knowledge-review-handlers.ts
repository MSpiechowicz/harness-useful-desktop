import type { KnowledgeComment } from '../bindings/KnowledgeComment';
import type { KnowledgeDocStatus } from '../bindings/KnowledgeDocStatus';
import type { KnowledgeDocument } from '../bindings/KnowledgeDocument';
import type { KnowledgeId } from '../bindings/KnowledgeId';
import type { KnowledgeOutcome } from '../bindings/KnowledgeOutcome';
import type { KnowledgeRefusalCode } from '../bindings/KnowledgeRefusalCode';
import type { KnowledgeTransition } from '../bindings/KnowledgeTransition';
import type { KnowledgeTransitioned } from '../bindings/KnowledgeTransitioned';
import { idKey } from '../knowledge-id';
import { IpcFailure } from '../transport';
import type { MockKnowledge } from './knowledge-handlers';
import { compareOf, ME, sameName } from './knowledge-review';
import type { Handlers } from './types';

type ReviewCommands =
  | 'knowledge_doc_transition'
  | 'knowledge_doc_review'
  | 'knowledge_doc_history'
  | 'knowledge_doc_comments'
  | 'knowledge_doc_comment_reply'
  | 'knowledge_doc_comment_resolve'
  | 'knowledge_doc_comment_delete'
  | 'knowledge_doc_compare'
  | 'knowledge_doc_download'
  | 'knowledge_reveal_download'
  | 'knowledge_approval_get'
  | 'knowledge_approval_set'
  | 'knowledge_held_request_changes'
  | 'knowledge_held_follow_up_available';

/** The chat the preview's first proposal was made in: the only one that is open, so "Request changes" reaches its agent. */
const LIVE_SESSION = 'a1b2c3';

const HISTORY_LIMIT = 200;

/** Where the preview "saves" a download: it has no dialog. */
const DOWNLOADS = '/home/demo/Downloads/';

/** The states each transition applies to. */
const FROM: Record<KnowledgeTransition, KnowledgeDocStatus[]> = {
  ask: ['draft', 'changesRequested'],
  requestChanges: ['waiting'],
  approve: ['waiting'],
  goLive: ['draft', 'waiting', 'changesRequested'],
  takeBack: ['waiting'],
};

const done = <T>(value: T): KnowledgeOutcome<T> => ({ type: 'done', value });

const refuse = (code: KnowledgeRefusalCode, message: string): { type: 'refused'; code: KnowledgeRefusalCode; message: string } => ({ type: 'refused', code, message });

/** The preview's Harness Knowledge predates drafts (`?knowledge=older`): the server answers a usage error, which the app reports as `unsupported`. */
function needDrafts(store: MockKnowledge): void {
  if (!store.drafts) {
    throw IpcFailure.from({ code: 'unsupported', message: 'this harness-knowledge predates drafts, so update Harness Knowledge' });
  }
}

/** The row with this id, any state. A remote row cannot be read while the remote is unreachable. */
function locate(store: MockKnowledge, id: KnowledgeId): KnowledgeDocument | undefined {
  return store.documents.some((doc) => idKey(doc.id) === idKey(id)) ? store.find(id, true) : undefined;
}

/** The row a review acts on: the pending version when the id is a live document that has one. */
function targetOf(store: MockKnowledge, found: KnowledgeDocument): KnowledgeDocument {
  return (found.status === 'live' ? store.pendingOf(found) : undefined) ?? found;
}

function replaceRow(store: MockKnowledge, row: KnowledgeDocument, next: KnowledgeDocument): KnowledgeDocument {
  store.documents[store.documents.indexOf(row)] = next;

  return next;
}

/** A pending version replaces its live document, which keeps its id, links and who added it: the rounds of the version join its history. */
function promote(store: MockKnowledge, row: KnowledgeDocument): KnowledgeDocument {
  const live = store.documents.find((doc) => row.pendingOf != null && idKey(doc.id) === idKey(row.pendingOf));

  if (!live) {
    return replaceRow(store, row, { ...row, status: 'live', revision: row.revision + 1 });
  }

  store.documents = store.documents.filter((doc) => doc !== row);
  store.review.mergeHistory(row.id, live.id);

  const { title, sourceName, format, bytes, chunkCount, contentSha256, fileSha256, updatedBy } = row;

  return replaceRow(store, live, { ...live, title, sourceName, format, bytes, chunkCount, contentSha256, fileSha256, updatedBy, updatedAt: new Date().toISOString(), revision: live.revision + 1 });
}

/** What refuses any change before it is looked at: a role that may not change the team database, and a document that moved on since it was shown. */
function lockedOrStale(scenario: string, row: KnowledgeDocument, expect: number): ReturnType<typeof refuse> | null {
  if (scenario === 'readonly') {
    return refuse('readOnly', 'this role may not change the team database');
  }

  return expect === row.revision ? null : refuse('changed', 'the document changed since it was shown');
}

/** The refusals the server checks before it moves a document, in its order: the role and the revision, the approval switch, the state, who may approve and what a request needs. */
function refusalOf(store: MockKnowledge, scenario: string, row: KnowledgeDocument, action: KnowledgeTransition, expect: number, comment: string | null): ReturnType<typeof refuse> | null {
  const approval = store.review.approval;
  const reviews = action === 'approve' || action === 'requestChanges';
  const early = lockedOrStale(scenario, row, expect);

  if (early) {
    return early;
  }

  // Taking an ask back is the author's own business: approval has no say, and only they may do it.
  if (action === 'takeBack') {
    if (!FROM.takeBack.includes(row.status)) {
      return refuse('wrongState', `a document that is ${row.status} cannot take this step`);
    }

    return store.review.isAuthor(row, ME) ? null : refuse('notAuthor', 'only the one who wrote it can take the ask back');
  }

  if (action === 'goLive' && approval) {
    return refuse('approvalOn', 'team approval is on, so a teammate approves it');
  }

  if (action !== 'goLive' && !approval) {
    return refuse('approvalOff', 'team approval is off, so make it live');
  }

  if (!FROM[action].includes(row.status)) {
    return refuse('wrongState', `a document that is ${row.status} cannot take this step`);
  }

  if (reviews && store.review.isAuthor(row, ME)) {
    return refuse('selfApproval', 'a teammate reviews what you wrote');
  }

  return action === 'requestChanges' && !comment?.trim() ? refuse('commentRequired', 'say what should change') : null;
}

function transition(store: MockKnowledge, scenario: string, id: KnowledgeId, action: KnowledgeTransition, expect: number, comment: string | null): KnowledgeOutcome<KnowledgeTransitioned> {
  needDrafts(store);

  const found = locate(store, id);

  if (!found) {
    return refuse('notFound', 'there is no such document');
  }

  const row = targetOf(store, found);
  const refused = refusalOf(store, scenario, row, action, expect, comment);

  if (refused) {
    return refused;
  }

  const at = new Date().toISOString();
  let next: KnowledgeDocument;

  if (action === 'approve' || action === 'goLive') {
    next = promote(store, row);
    store.review.record(next.id, action === 'approve' ? 'approved' : 'wentLive', ME);
  } else if (action === 'takeBack') {
    next = replaceRow(store, row, { ...row, status: 'draft', revision: row.revision + 1 });
    store.review.record(next.id, 'withdrawn', ME);
  } else {
    const status: KnowledgeDocStatus = action === 'ask' ? 'waiting' : 'changesRequested';

    next = replaceRow(store, row, { ...row, status, revision: row.revision + 1, lastChange: action === 'requestChanges' ? { by: ME, at, comment: comment?.trim() ?? null } : row.lastChange });
    store.review.record(next.id, action === 'ask' ? 'asked' : 'changesRequested', ME, action === 'ask' ? null : (comment?.trim() ?? null));

    if (action === 'ask') {
      store.review.asked.set(idKey(next.id), [...(store.review.asked.get(idKey(next.id)) ?? []), next.revision]);
    }
  }

  store.changed();

  return done({ id: next.id, status: next.status, revision: next.revision, promoted: row.pendingOf != null && (action === 'approve' || action === 'goLive') });
}

function addComment(store: MockKnowledge, row: KnowledgeDocument, fields: Partial<KnowledgeComment> & { body: string }): KnowledgeComment {
  const added: KnowledgeComment = { id: store.review.nextCommentId(), reviewId: 1, section: null, occurrence: null, side: null, from: null, to: null, by: ME, at: new Date().toISOString(), parentId: null, resolvedAt: null, resolvedBy: null, revision: row.revision, outdated: false, ...fields };

  store.review.comments.set(idKey(row.id), [...(store.review.comments.get(idKey(row.id)) ?? []), added]);

  return added;
}

/** The comment with this id and the row it is on. */
function commentOf(store: MockKnowledge, id: KnowledgeId): { rowId: string; comment: KnowledgeComment } | undefined {
  for (const [rowId, comments] of store.review.comments) {
    const comment = comments.find((candidate) => idKey(candidate.id) === idKey(id));

    if (comment) {
      return { rowId, comment };
    }
  }

  return undefined;
}

function changeComment(store: MockKnowledge, found: { rowId: string; comment: KnowledgeComment }, next: KnowledgeComment): KnowledgeOutcome<{ comment: KnowledgeComment }> {
  store.review.comments.set(found.rowId, (store.review.comments.get(found.rowId) ?? []).map((comment) => (comment === found.comment ? next : comment)));
  store.changed();

  return done({ comment: next });
}

/** Comments and replies are anybody's to write, the author's included, but not on a team database the user may not change. */
function commenting(store: MockKnowledge, scenario: string, id: KnowledgeId, commentId: KnowledgeId): { found: { rowId: string; comment: KnowledgeComment } } | ReturnType<typeof refuse> {
  needDrafts(store);

  if (scenario === 'readonly') {
    return refuse('readOnly', 'this role may not change the team database');
  }

  const found = commentOf(store, commentId);

  return found && locate(store, id) ? { found } : refuse('notFound', 'there is no such comment');
}

/**
 * The review and approval commands over the in-memory database: transitions with the server's refusals (including `selfApproval` for what the person the preview
 * is signed in as wrote), reviews with line comments, compare, history, the team approval switch and the request for changes on a proposal.
 */
export function knowledgeReviewHandlers(store: MockKnowledge, scenario: () => string): Pick<Handlers, ReviewCommands> {
  return {
    knowledge_doc_transition: ({ id, action, revision, comment }) => transition(store, scenario(), id, action, revision, comment),

    knowledge_doc_review: ({ id, revision, review }) => {
      needDrafts(store);

      const found = locate(store, id);

      if (!found) {
        return refuse('notFound', 'there is no such document');
      }

      const row = targetOf(store, found);
      const action: KnowledgeTransition | null = review.verdict === 'approve' ? 'approve' : review.verdict === 'requestChanges' ? 'requestChanges' : null;

      // A comment-only review changes no state, so only the role and the revision can refuse it.
      const outcome = action ? transition(store, scenario(), id, action, revision, review.body ?? null) : (lockedOrStale(scenario(), row, revision) ?? done({ id: row.id, status: row.status, revision: row.revision, promoted: false }));

      if (outcome.type === 'refused') {
        return outcome;
      }

      // The comments belong to the document the review ended on: the live id after a promote.
      const reviewed = store.documents.find((doc) => idKey(doc.id) === idKey(outcome.value.id)) ?? row;
      const overall = review.body?.trim();

      if (overall) {
        addComment(store, reviewed, { body: overall, revision });
      }

      review.comments.forEach((line) => addComment(store, reviewed, { section: line.section, occurrence: line.occurrence ?? 1, side: line.side, from: line.from, to: line.to, body: line.body.trim(), revision }));
      store.changed();

      return outcome;
    },

    knowledge_doc_history: ({ id }) => {
      needDrafts(store);

      if (!locate(store, id)) {
        throw IpcFailure.from({ code: 'notFound', message: `no document with id ${id}` });
      }

      const events = store.review.events.get(idKey(id)) ?? [];

      return { events: events.slice(0, HISTORY_LIMIT), total: events.length };
    },

    knowledge_doc_comments: ({ id }) => {
      needDrafts(store);

      const found = locate(store, id);
      const row = found ? targetOf(store, found) : undefined;

      return (row ? (store.review.comments.get(idKey(row.id)) ?? []) : []).map((comment) => ({ ...comment, outdated: comment.revision < (row?.revision ?? 0) }));
    },

    knowledge_doc_comment_reply: ({ id, commentId, body }) => {
      const asked = commenting(store, scenario(), id, commentId);

      if ('type' in asked) {
        return asked;
      }

      const row = targetOf(store, locate(store, id) as KnowledgeDocument);
      const reply = addComment(store, row, { body: body.trim(), reviewId: null, parentId: asked.found.comment.parentId ?? asked.found.comment.id });
      store.changed();

      return done({ comment: reply });
    },

    knowledge_doc_comment_resolve: ({ id, commentId, resolved }) => {
      const asked = commenting(store, scenario(), id, commentId);

      if ('type' in asked) {
        return asked;
      }

      return changeComment(store, asked.found, { ...asked.found.comment, resolvedAt: resolved ? new Date().toISOString() : null, resolvedBy: resolved ? ME : null });
    },

    // Only the writer's own comment: a root takes its replies with it, a reply goes alone, the history is untouched.
    knowledge_doc_comment_delete: ({ id, commentId }) => {
      const asked = commenting(store, scenario(), id, commentId);

      if ('type' in asked) {
        return asked;
      }

      const { rowId, comment } = asked.found;

      if (!sameName(comment.by, ME)) {
        return refuse('notAuthor', 'only the one who wrote a comment can delete it');
      }

      const gone = (store.review.comments.get(rowId) ?? []).filter((candidate) => candidate === comment || (comment.parentId == null && candidate.parentId != null && idKey(candidate.parentId) === idKey(comment.id)));

      store.review.comments.set(rowId, (store.review.comments.get(rowId) ?? []).filter((candidate) => !gone.includes(candidate)));
      store.changed();

      return done({ deleted: gone.map((candidate) => candidate.id) });
    },

    knowledge_doc_compare: ({ id, since }) => {
      needDrafts(store);

      const found = locate(store, id);
      const row = found ? targetOf(store, found) : undefined;

      if (!row || (row.status === 'live' && row.pendingOf == null)) {
        return refuse('notFound', 'there is no pending version of that document');
      }

      if (since !== null && !(store.review.asked.get(idKey(row.id)) ?? []).includes(since)) {
        return refuse('sinceUnavailable', 'that version was not kept');
      }

      return done(compareOf(row, store.documents.find((doc) => row.pendingOf != null && idKey(doc.id) === idKey(row.pendingOf)), since));
    },

    knowledge_doc_download: ({ id, name, live }) => {
      needDrafts(store);

      const found = locate(store, id);
      const row = found && !live ? targetOf(store, found) : found;

      // The preview has no dialog: it saves where the user's downloads would be.
      return row ? done({ path: `${DOWNLOADS}${name ?? 'document.md'}`, original: row.origin === 'user' }) : refuse('notFound', 'there is no such document');
    },

    // Only a file the preview saved: the app reveals the paths the save dialog returned and no others.
    knowledge_reveal_download: ({ path }) => {
      if (!path.startsWith(DOWNLOADS) || path.includes('..')) {
        throw IpcFailure.from({ code: 'forbidden', message: 'refused by the access rules' });
      }

      return null;
    },

    knowledge_approval_get: () => {
      if (!store.drafts) {
        throw IpcFailure.from({ code: 'unsupported', message: 'this harness-knowledge predates team approval, so update Harness Knowledge' });
      }

      return { approval: store.review.approval, place: 'remote', writable: store.offline ? null : scenario() !== 'readonly', changedBy: store.review.changedBy, changedAt: store.review.changedAt };
    },

    knowledge_approval_set: ({ on }) => {
      needDrafts(store);

      if (scenario() === 'readonly') {
        return refuse('readOnly', 'this role may not change the team database');
      }

      store.review.approval = on;
      store.review.changedBy = ME;
      store.review.changedAt = new Date().toISOString();
      store.changed();

      return done({ approval: on, place: 'remote', writable: true, changedBy: ME, changedAt: store.review.changedAt });
    },

    knowledge_held_request_changes: ({ id, revision, comment }) => {
      const item = store.held.find((candidate) => candidate.id === id);
      const failed = (type: 'notFound' | 'changed' | 'commentRequired', message: string) => ({ decided: { type, message }, delivered: false });

      if (!item) {
        return failed('notFound', 'that proposal was decided already');
      }

      if (revision !== null && revision !== item.revision) {
        return failed('changed', 'the proposal changed since it was shown');
      }

      if (!comment.trim()) {
        return failed('commentRequired', 'say what should change');
      }

      store.held = store.held.map((candidate) => (candidate === item ? { ...item, status: 'changesRequested', comment: comment.trim() } : candidate));
      store.changed();

      return { decided: { type: 'changesRequested' }, delivered: item.session === LIVE_SESSION };
    },

    // The chat is open in the proposal's own project only: a session id alone is a name any proposal can carry.
    knowledge_held_follow_up_available: ({ session, projectPath }) => session === LIVE_SESSION && projectPath != null && store.held.some((item) => item.session === session && item.projectPath === projectPath),
  };
}
