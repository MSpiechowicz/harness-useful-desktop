import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import type { KnowledgeComment } from '$lib/ipc/bindings/KnowledgeComment';
import type { KnowledgeCompare } from '$lib/ipc/bindings/KnowledgeCompare';
import type { KnowledgeHistory } from '$lib/ipc/bindings/KnowledgeHistory';
import type { KnowledgeHistoryEvent } from '$lib/ipc/bindings/KnowledgeHistoryEvent';
import type { KnowledgeOutcome } from '$lib/ipc/bindings/KnowledgeOutcome';
import type { KnowledgeRefusalCode } from '$lib/ipc/bindings/KnowledgeRefusalCode';
import type { KnowledgeReviewSubmit } from '$lib/ipc/bindings/KnowledgeReviewSubmit';
import type { KnowledgeTransition } from '$lib/ipc/bindings/KnowledgeTransition';
import type { KnowledgeTransitioned } from '$lib/ipc/bindings/KnowledgeTransitioned';
import { ipc } from '$lib/ipc/client';
import { idFromKey, idKey, type KnowledgeId } from '$lib/ipc/knowledge-id';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { writeFailureText } from '$lib/utils/knowledge-failure';

import { knowledgeDocs } from './knowledgeDocs.svelte';
import { knowledgeViewer, sameName } from './knowledgeViewer.svelte';
import { type ToastAction, toasts } from './toasts.svelte';
import { ViewLoader } from './view-loader.svelte';

/** The title and body of the toast each refusal of the server gets. `refused` says what the server said instead. */
const REFUSALS: Record<Exclude<KnowledgeRefusalCode, 'refused'>, { title: MessageKey; body: MessageKey }> = {
  notFound: { title: 'knowledge.drafts.refusal.notFound.title', body: 'knowledge.drafts.refusal.notFound.body' },
  changed: { title: 'knowledge.drafts.refusal.changed.title', body: 'knowledge.drafts.refusal.changed.body' },
  wrongState: { title: 'knowledge.drafts.refusal.wrongState.title', body: 'knowledge.drafts.refusal.wrongState.body' },
  approvalOn: { title: 'knowledge.drafts.refusal.approvalOn.title', body: 'knowledge.drafts.refusal.approvalOn.body' },
  approvalOff: { title: 'knowledge.drafts.refusal.approvalOff.title', body: 'knowledge.drafts.refusal.approvalOff.body' },
  selfApproval: { title: 'knowledge.drafts.refusal.selfApproval.title', body: 'knowledge.drafts.refusal.selfApproval.body' },
  notAuthor: { title: 'knowledge.drafts.refusal.notAuthor.title', body: 'knowledge.drafts.refusal.notAuthor.body' },
  noName: { title: 'knowledge.drafts.refusal.noName.title', body: 'knowledge.drafts.refusal.noName.body' },
  readOnly: { title: 'knowledge.drafts.refusal.readOnly.title', body: 'knowledge.drafts.refusal.readOnly.body' },
  commentRequired: { title: 'knowledge.drafts.refusal.commentRequired.title', body: 'knowledge.drafts.refusal.commentRequired.body' },
  sinceUnavailable: { title: 'knowledge.drafts.refusal.sinceUnavailable.title', body: 'knowledge.drafts.refusal.sinceUnavailable.body' },
  localPath: { title: 'knowledge.drafts.refusal.localPath.title', body: 'knowledge.drafts.refusal.localPath.body' },
};

/** Says in a toast (bottom right) why the server refused a step of the review, in the app's words. `changed` may offer a way on (`again`). */
export function sayRefusal(code: KnowledgeRefusalCode, message: string, again?: ToastAction): void {
  if (code === 'refused') {
    toasts.push({ title: t('knowledge.drafts.refusal.refused.title'), description: message, tone: 'danger' });

    return;
  }

  const text = REFUSALS[code];

  toasts.push({
    title: t(text.title),
    description: t(text.body),
    tone: code === 'changed' || code === 'wrongState' || code === 'notFound' ? 'warning' : 'danger',
    action: code === 'changed' ? again : undefined,
  });
}

/** The version a step acts on: the document (or the pending update of a live one) as it was shown. */
export interface ReviewTarget {
  id: KnowledgeId;
  /** Given back to the server, so a version that changed meanwhile is not decided blindly. */
  revision: number;
  title: string;
  /** Who wrote this version: a `selfApproval` refusal teaches the user's own name from it, when the asker of the review is not somebody else. */
  author: string | null;
}

const DONE_TOASTS: Record<KnowledgeTransition, MessageKey> = {
  ask: 'knowledge.drafts.toast.asked',
  requestChanges: 'knowledge.drafts.toast.changesRequested',
  approve: 'knowledge.drafts.toast.approved',
  goLive: 'knowledge.drafts.toast.wentLive',
  takeBack: 'knowledge.drafts.toast.tookBack',
};

const DONE_BODIES: Record<KnowledgeTransition, MessageKey> = {
  ask: 'knowledge.drafts.toast.asked.body',
  requestChanges: 'knowledge.drafts.toast.changesRequested.body',
  approve: 'knowledge.drafts.toast.approved.body',
  goLive: 'knowledge.drafts.toast.wentLive.body',
  takeBack: 'knowledge.drafts.toast.tookBack.body',
};

/** The history, comments and comparison of the document in the panel or the review view, and the steps of the review. */
export class KnowledgeReviewStore {
  /** The history of the version a review acts on (the pending row of a live document). */
  readonly history: ViewLoader<KnowledgeHistory, string>;
  /** The history of the live document that has a pending version, which the History section joins with the pending version's. */
  readonly liveHistory: ViewLoader<KnowledgeHistory, string>;
  readonly comments: ViewLoader<KnowledgeComment[], string>;
  /** The pending version against its live document. */
  readonly compare: ViewLoader<KnowledgeOutcome<KnowledgeCompare>, string>;
  /** The pending version against the version that was asked at a revision (the review view's "since your review"). */
  readonly since: ViewLoader<KnowledgeOutcome<KnowledgeCompare>, string>;
  /** The documents a step is being made on now. */
  busy = $state.raw<ReadonlySet<string>>(new Set());

  constructor(private readonly transport: Transport) {
    this.history = new ViewLoader((key) => transport.invoke('knowledge_doc_history', { id: idFromKey(key) }));
    this.liveHistory = new ViewLoader((key) => transport.invoke('knowledge_doc_history', { id: idFromKey(key) }));
    this.comments = new ViewLoader((key) => transport.invoke('knowledge_doc_comments', { id: idFromKey(key) }));
    this.compare = new ViewLoader((key) => transport.invoke('knowledge_doc_compare', { id: idFromKey(key), since: null }));
    this.since = new ViewLoader((key) => {
      const [id, revision] = key.split('|');

      return transport.invoke('knowledge_doc_compare', { id: idFromKey(id), since: Number(revision) });
    });
  }

  /** Reads what is on screen again without a loading state: the database changed, or a step changed it. */
  refresh(): Promise<unknown> {
    return Promise.all([
      this.history.refresh({ quiet: true }),
      this.liveHistory.refresh({ quiet: true }),
      this.comments.refresh({ quiet: true }),
      this.compare.refresh({ quiet: true }),
      this.since.refresh({ quiet: true }),
    ]);
  }

  /** The events of the version `id` once they were read for it, else null (not read yet, or still those of another document). */
  eventsOf(id: KnowledgeId): readonly KnowledgeHistoryEvent[] | null {
    return this.eventsFrom(this.history, id);
  }

  /** The events of the live document `id` that has a pending version, as `eventsOf` answers them. */
  liveEventsOf(id: KnowledgeId): readonly KnowledgeHistoryEvent[] | null {
    return this.eventsFrom(this.liveHistory, id);
  }

  private eventsFrom(loader: ViewLoader<KnowledgeHistory, string>, id: KnowledgeId): readonly KnowledgeHistoryEvent[] | null {
    // Read first: what is read is what a derived value depends on, and whether the data is for `id` does not change by itself.
    const data = loader.data;

    return loader.holds(idKey(id), null) ? (data?.events ?? null) : null;
  }

  /**
   * What a loader read for `key`, once it is ready for it, else null. A loader keeps the data of the document it left while the next one loads, so the key alone does not tell: only a
   * read that ended for this key is the answer for it (never another document's comparison, comments or history to decide on).
   */
  private readyFor<T>(loader: ViewLoader<T, string>, key: string): T | null {
    // Read first, as `eventsFrom` does.
    const data = loader.data;
    const status = loader.status;

    return status === 'ready' && loader.holds(key, null) ? data : null;
  }

  /** What the server answered to the comparison of the version `id`, once it was read for it, else null (not read yet, still loading, failed, or still that of another document). */
  compareOutcome(id: KnowledgeId): KnowledgeOutcome<KnowledgeCompare> | null {
    return this.readyFor(this.compare, idKey(id));
  }

  /** The comparison of the version `id` that is on screen once it was read for it, else null (not read yet, failed, refused, or still that of another document). */
  shownCompare(id: KnowledgeId): KnowledgeCompare | null {
    const outcome = this.compareOutcome(id);

    return outcome?.type === 'done' ? outcome.value : null;
  }

  /** Whether reading the comparison of the version `id` failed. */
  compareFailed(id: KnowledgeId): boolean {
    const status = this.compare.status;

    return status === 'error' && this.compare.holds(idKey(id), null);
  }

  /** What the server answered to the comparison of the version `id` with its row at `revision`, once it was read for that pair, else null. */
  sinceOutcome(id: KnowledgeId, revision: number): KnowledgeOutcome<KnowledgeCompare> | null {
    return this.readyFor(this.since, `${idKey(id)}|${revision}`);
  }

  /** The saved comments of the version `id` once they were read for it, else null (not read yet, or still those of another document). */
  commentsOf(id: KnowledgeId): KnowledgeComment[] | null {
    return this.readyFor(this.comments, idKey(id));
  }

  private async changed(): Promise<void> {
    await Promise.all([knowledgeDocs.refresh(), this.refresh()]);
  }

  private async guarded<T>(target: ReviewTarget, failed: MessageKey, run: () => Promise<KnowledgeOutcome<T>>, again?: ToastAction): Promise<KnowledgeOutcome<T> | null> {
    const key = idKey(target.id);

    if (this.busy.has(key)) {
      return null;
    }

    this.busy = new Set([...this.busy, key]);

    try {
      const outcome = await run();

      if (outcome.type === 'refused') {
        sayRefusal(outcome.code, outcome.message, again);
        // What the refusal says about the document (it changed, it moved on) is read again, so the panel shows its truth.
        await this.changed();
        this.learnFrom(outcome.code, target);
      }

      return outcome;
    } catch (error) {
      toasts.push({ title: t(failed), description: writeFailureText(error), tone: 'danger' });

      return null;
    } finally {
      this.busy = new Set([...this.busy].filter((each) => each !== key));
    }
  }

  private learnFrom(code: KnowledgeRefusalCode, target: ReviewTarget): void {
    if (code === 'selfApproval') {
      this.learnSelf(target);
    } else if (code === 'noName') {
      knowledgeViewer.learnNoName();
    }
  }

  /** The server refuses the author of a version and the one who asked for its review alike: the author is the user only when no other person asked. */
  private learnSelf(target: ReviewTarget): void {
    const events = this.eventsOf(target.id);
    const asker = events?.find((event) => event.kind === 'asked')?.by ?? null;

    if (events !== null && (asker === null || sameName(asker, target.author))) {
      knowledgeViewer.learn(target.author);
    }
  }

  /** Moves a version one step (ask, request changes, approve, go live, take the ask back). Answers whether the server did it; a refusal says why in a toast. */
  async transition(target: ReviewTarget, action: KnowledgeTransition, comment: string | null = null): Promise<KnowledgeTransitioned | null> {
    const outcome = await this.guarded(target, 'knowledge.drafts.toast.stepFailed', () =>
      this.transport.invoke('knowledge_doc_transition', { id: target.id, action, revision: target.revision, comment }),
    );

    if (outcome?.type !== 'done') {
      return null;
    }

    toasts.push({ title: t(DONE_TOASTS[action]), description: t(DONE_BODIES[action], { title: target.title }), tone: 'success' });
    await this.changed();

    return outcome.value;
  }

  /**
   * Submits a review: the verdict with the overall comment and the line comments. Answers what became of the document, or null when it was refused.
   * `target.revision` is the one the line comments were written on, so a document that changed since is refused as `changed`, and `again` is what the toast offers then.
   */
  async submit(target: ReviewTarget, review: KnowledgeReviewSubmit, again?: ToastAction): Promise<KnowledgeTransitioned | null> {
    const outcome = await this.guarded(target, 'knowledge.drafts.toast.reviewFailed', () => this.transport.invoke('knowledge_doc_review', { id: target.id, revision: target.revision, review }), again);

    if (outcome?.type !== 'done') {
      return null;
    }

    const key: MessageKey =
      review.verdict === 'approve' ? 'knowledge.drafts.toast.approved' : review.verdict === 'requestChanges' ? 'knowledge.drafts.toast.changesRequested' : 'knowledge.drafts.toast.reviewed';
    const body: MessageKey =
      review.verdict === 'approve'
        ? 'knowledge.drafts.toast.approved.body'
        : review.verdict === 'requestChanges'
          ? 'knowledge.drafts.toast.changesRequested.body'
          : 'knowledge.drafts.toast.reviewed.body';

    toasts.push({ title: t(key), description: t(body, { title: target.title }), tone: 'success' });
    await this.changed();

    return outcome.value;
  }

  /** Answers a comment, or resolves it (and opens it again). */
  async reply(target: ReviewTarget, commentId: KnowledgeId, body: string): Promise<boolean> {
    return this.say(target, () => this.transport.invoke('knowledge_doc_comment_reply', { id: target.id, commentId, body }));
  }

  async resolve(target: ReviewTarget, commentId: KnowledgeId, resolved: boolean): Promise<boolean> {
    return this.say(target, () => this.transport.invoke('knowledge_doc_comment_resolve', { id: target.id, commentId, resolved }));
  }

  /** Deletes a comment the user wrote, with its replies when it starts a thread. The comments are read again, so what is gone disappears. */
  async remove(target: ReviewTarget, commentId: KnowledgeId): Promise<boolean> {
    return this.say(target, () => this.transport.invoke('knowledge_doc_comment_delete', { id: target.id, commentId }), 'knowledge.drafts.toast.commentDeleteFailed');
  }

  private async say(target: ReviewTarget, run: () => Promise<KnowledgeOutcome<unknown>>, failed: MessageKey = 'knowledge.drafts.toast.commentFailed'): Promise<boolean> {
    const outcome = await this.guarded(target, failed, run);

    if (outcome?.type !== 'done') {
      return false;
    }

    await this.changed();

    return true;
  }

  /** Lets the user save the original file of a version (the live one when `live`). Says in a toast where it went, with a way to show the folder. */
  async download(target: Pick<ReviewTarget, 'id' | 'title'>, name: string, live: boolean): Promise<void> {
    try {
      const outcome = await this.transport.invoke('knowledge_doc_download', { id: target.id, name, live });

      // The user closed the dialog.
      if (outcome === null) {
        return;
      }

      if (outcome.type === 'refused') {
        sayRefusal(outcome.code, outcome.message);

        return;
      }

      const { path, original } = outcome.value;

      toasts.push({
        title: t('knowledge.drafts.toast.downloaded', { name: path.split(/[\\/]/).filter(Boolean).at(-1) ?? name }),
        description: original ? path : t('knowledge.drafts.toast.downloadedText'),
        tone: 'success',
        action: { label: t('knowledge.drafts.toast.showInFolder'), run: () => void this.reveal(path) },
      });
    } catch (error) {
      toasts.push({ title: t('knowledge.drafts.toast.downloadFailed'), description: writeFailureText(error), tone: 'danger' });
    }
  }

  /** Shows a file saved by `download` in the file manager. The host reveals only what the save dialog returned. */
  private async reveal(path: string): Promise<void> {
    try {
      await this.transport.invoke('knowledge_reveal_download', { path });
    } catch (error) {
      toasts.push({ title: t('components.action.revealFailed'), description: IpcFailure.from(error).message, tone: 'danger' });
    }
  }

  /** Forgets what was read, e.g. for a test. */
  reset(): void {
    this.history.sync(null, null);
    this.liveHistory.sync(null, null);
    this.comments.sync(null, null);
    this.compare.sync(null, null);
    this.since.sync(null, null);
    this.busy = new Set();
  }
}

export const knowledgeReview = new KnowledgeReviewStore(ipc);
