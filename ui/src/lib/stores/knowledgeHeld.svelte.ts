import type { Sort } from '$lib/components/table';
import { type MessageKey, t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeDecided } from '$lib/ipc/bindings/KnowledgeDecided';
import type { KnowledgeDecision } from '$lib/ipc/bindings/KnowledgeDecision';
import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';
import type { KnowledgeHeldPage } from '$lib/ipc/bindings/KnowledgeHeldPage';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';
import { writeFailureText } from '$lib/utils/knowledge-failure';
import { localPathToast, withoutFolderPaths } from '$lib/utils/knowledge-local-path';

import { KNOWLEDGE_PAGE_SIZE, scopeOf } from './knowledgeDocs.svelte';
import { toasts } from './toasts.svelte';
import { ViewLoader } from './view-loader.svelte';

/** What the review queue asks for: one page of the proposals that answer the search and the scope, sorted by a column. */
export interface HeldRequest {
  query: string;
  page: number;
  scope: string;
  sort: Sort;
}

export function heldKey(request: HeldRequest): string {
  return JSON.stringify(request);
}

/** How a decision ended for the view: the row is gone (`left`), or it stays and the user reads it again (`stay`). */
export type DecisionEnd = 'left' | 'stay';

/** The documents agents proposed, waiting for the user: the page on screen, the proposal in the panel and the two decisions. */
export class KnowledgeHeldStore {
  readonly list: ViewLoader<KnowledgeHeldPage, string>;
  readonly detail: ViewLoader<KnowledgeHeldItem, string>;
  /** Whether the chat of a proposal's agent is open here, keyed by `followUpKey` (the chat is found in the folder the proposal was about). */
  readonly followUp: ViewLoader<boolean, string>;
  /** The proposals being decided now. */
  busy = $state.raw<ReadonlySet<string>>(new Set());

  constructor(private readonly transport: Transport) {
    this.list = new ViewLoader((key) => {
      const { query, page, scope, sort } = JSON.parse(key) as HeldRequest;
      const search = query.trim();

      return transport.invoke('knowledge_held_list', {
        offset: page * KNOWLEDGE_PAGE_SIZE,
        limit: KNOWLEDGE_PAGE_SIZE,
        query: search === '' ? null : search,
        scope: scopeOf(scope),
        sort: sort.key,
        order: sort.desc ? 'desc' : 'asc',
      });
    });
    this.detail = new ViewLoader((id) => transport.invoke('knowledge_held_get', { id }), { goneIsFinal: true });
    this.followUp = new ViewLoader((key) => {
      const [session, projectPath] = JSON.parse(key) as [string, string | null];

      return transport.invoke('knowledge_held_follow_up_available', { session, projectPath });
    });
  }

  refresh(): Promise<unknown> {
    return Promise.all([this.list.refresh({ quiet: true }), this.detail.refresh({ quiet: true }), this.followUp.refresh({ quiet: true })]);
  }

  /**
   * Approves or rejects the version of a proposal the user was shown. A refusal is an answer, not an error: each says what happened in a toast,
   * and only a carried out decision (or a proposal somebody decided already) takes the row away.
   */
  async decide(item: KnowledgeHeldItem, decision: KnowledgeDecision, comment: string | null = null): Promise<DecisionEnd> {
    if (this.busy.has(item.id)) {
      return 'stay';
    }

    this.busy = new Set([...this.busy, item.id]);

    try {
      const reason = decision === 'reject' ? comment?.trim() : '';
      const answer = await this.transport.invoke('knowledge_held_decide', { id: item.id, decision, revision: item.revision, ...(reason ? { comment: reason } : {}) });

      this.say(item, answer);
      await this.refresh();

      return answer.type === 'approved' || answer.type === 'rejected' || answer.type === 'notFound' ? 'left' : 'stay';
    } catch (error) {
      toasts.push({ title: t(decision === 'approve' ? 'knowledge.held.toast.approveFailed' : 'knowledge.held.toast.rejectFailed'), description: writeFailureText(error), tone: 'danger' });

      return 'stay';
    } finally {
      this.busy = new Set([...this.busy].filter((id) => id !== item.id));
    }
  }

  private say(item: KnowledgeHeldItem, answer: KnowledgeDecided): void {
    switch (answer.type) {
      case 'approved': {
        const key = approvedToast(answer);

        toasts.push({ title: t(key), description: answer.title, tone: 'success' });

        return;
      }
      case 'rejected':
        toasts.push({ title: t('knowledge.held.toast.rejected'), description: item.title, tone: 'success' });

        return;
      case 'notFound':
        toasts.push({ title: t('knowledge.held.toast.gone'), description: item.title, tone: 'info' });

        return;
      case 'changed':
        toasts.push({ title: t('knowledge.held.toast.changed'), description: t('knowledge.held.toast.changedBody', { title: item.title }), tone: 'warning' });

        return;
      case 'changesRequested':
        toasts.push({ title: t('knowledge.held.toast.changesRequested'), description: item.title, tone: 'success' });

        return;
      case 'commentRequired':
        toasts.push({ title: t('knowledge.drafts.refusal.commentRequired.title'), description: t('knowledge.drafts.refusal.commentRequired.body'), tone: 'danger' });

        return;
      case 'localPath':
        toasts.push({ ...localPathToast(item.links), tone: 'danger' });

        return;
      case 'refused':
        toasts.push({ title: t('knowledge.held.toast.refused'), description: withoutFolderPaths(answer.message, item.links), tone: 'danger' });
    }
  }

  /**
   * Asks the agent of a proposal for changes: it stays in the queue as `changesRequested` with the comment. The toast says whether the comment also
   * reached the agent's open chat. Answers whether it was done.
   */
  async requestChanges(item: KnowledgeHeldItem, comment: string): Promise<boolean> {
    if (this.busy.has(item.id)) {
      return false;
    }

    this.busy = new Set([...this.busy, item.id]);

    try {
      const answer = await this.transport.invoke('knowledge_held_request_changes', { id: item.id, revision: item.revision, comment: comment.trim() });

      if (answer.decided.type === 'changesRequested') {
        toasts.push({
          title: t('knowledge.held.toast.changesRequested'),
          description: t(answer.delivered ? 'knowledge.held.toast.changesDelivered' : 'knowledge.held.toast.changesKept', { title: item.title }),
          tone: 'success',
        });
      } else {
        this.say(item, answer.decided);
      }

      await this.refresh();

      return answer.decided.type === 'changesRequested';
    } catch (error) {
      toasts.push({ title: t('knowledge.held.toast.changesFailed'), description: writeFailureText(error), tone: 'danger' });

      return false;
    } finally {
      this.busy = new Set([...this.busy].filter((id) => id !== item.id));
    }
  }

  /**
   * Rejects a proposal after a moment in which the toast offers to undo it: the server is asked once the toast is dismissed (its time ran out or the user closed
   * it), unless Undo was pressed (then `onundo` brings the row back). Other toasts pushing this one out of the stack do not count as a dismissal: the proposal is
   * rejected once the moment is over. Closing the app in that moment keeps the proposal.
   */
  rejectWithUndo(item: KnowledgeHeldItem, comment: string | null, onundo: () => void): void {
    let settled = false;
    let backstop: ReturnType<typeof setTimeout> | undefined;

    const settle = (undone: boolean) => {
      if (settled) {
        return;
      }

      settled = true;
      clearTimeout(backstop);

      if (undone) {
        onundo();
      } else {
        void this.rejectNow(item, comment, onundo);
      }
    };

    const id = toasts.push({
      title: t('knowledge.held.toast.rejected'),
      description: item.title,
      tone: 'success',
      duration: UNDO_MS,
      action: { label: t('knowledge.held.toast.undo'), run: () => settle(true) },
      onclose: () => settle(false),
    });

    // A toast pushed out of the stack can no longer be dismissed: once its moment is over the rejection goes ahead. A toast that is still shown (the pointer may
    // have paused it) decides by its own timer, which this only looks at again soon.
    const watch = (wait: number) => {
      backstop = setTimeout(() => {
        if (toasts.has(id)) {
          watch(UNDO_POLL_MS);
        } else {
          settle(false);
        }
      }, wait);
    };

    watch(UNDO_MS);
  }

  /** Rejects without the toast of `decide`, which `rejectWithUndo` already showed. A decision that did not take the proposal away brings its row back. */
  private async rejectNow(item: KnowledgeHeldItem, comment: string | null, onstay: () => void): Promise<void> {
    if (this.busy.has(item.id)) {
      return;
    }

    this.busy = new Set([...this.busy, item.id]);

    try {
      const reason = comment?.trim();
      const answer = await this.transport.invoke('knowledge_held_decide', { id: item.id, decision: 'reject', revision: item.revision, ...(reason ? { comment: reason } : {}) });

      if (answer.type !== 'rejected') {
        this.say(item, answer);
        onstay();
      }

      await this.refresh();
    } catch (error) {
      toasts.push({ title: t('knowledge.held.toast.rejectFailed'), description: writeFailureText(error), tone: 'danger' });
      onstay();
    } finally {
      this.busy = new Set([...this.busy].filter((each) => each !== item.id));
    }
  }
}

/** The key of `followUp` for a proposal: the chat of its agent and the folder it was about. */
export function followUpKey(item: { session?: string | null; projectPath?: string | null }): string | null {
  return item.session == null ? null : JSON.stringify([item.session, item.projectPath ?? null]);
}

/** How long the toast that follows a rejection offers to undo it, and how often the store looks again at one that is still shown after that. */
const UNDO_MS = 6000;
const UNDO_POLL_MS = 200;

/** The title of the toast an approval gets: what it did, and whether it stands as a draft or a pending update until a teammate approves it. */
function approvedToast(answer: Extract<KnowledgeDecided, { type: 'approved' }>): MessageKey {
  if (answer.status === 'pending') {
    return 'knowledge.held.toast.pending';
  }

  if (answer.status === 'added') {
    return answer.draft ? 'knowledge.held.toast.draft' : 'knowledge.held.toast.added';
  }

  return answer.status === 'updated' ? 'knowledge.held.toast.updated' : 'knowledge.held.toast.unchanged';
}

export const knowledgeHeld = new KnowledgeHeldStore(ipc);
