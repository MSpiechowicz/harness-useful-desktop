import { t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeDecided } from '$lib/ipc/bindings/KnowledgeDecided';
import type { KnowledgeDecision } from '$lib/ipc/bindings/KnowledgeDecision';
import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';
import type { KnowledgeHeldPage } from '$lib/ipc/bindings/KnowledgeHeldPage';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import { KNOWLEDGE_PAGE_SIZE } from './knowledgeDocs.svelte';
import { toasts } from './toasts.svelte';
import { ViewLoader } from './view-loader.svelte';

/** What the review queue asks the server for: one page of the proposals that answer the search. */
export interface HeldRequest {
  query: string;
  page: number;
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
  /** The proposals being decided now. */
  busy = $state.raw<ReadonlySet<string>>(new Set());

  constructor(private readonly transport: Transport) {
    this.list = new ViewLoader((key) => {
      const { query, page } = JSON.parse(key) as HeldRequest;
      const search = query.trim();

      return transport.invoke('knowledge_held_list', { offset: page * KNOWLEDGE_PAGE_SIZE, limit: KNOWLEDGE_PAGE_SIZE, query: search === '' ? null : search });
    });
    this.detail = new ViewLoader((id) => transport.invoke('knowledge_held_get', { id }), { goneIsFinal: true });
  }

  refresh(): Promise<unknown> {
    return Promise.all([this.list.refresh({ quiet: true }), this.detail.refresh({ quiet: true })]);
  }

  /**
   * Approves or rejects the version of a proposal the user was shown. A refusal is an answer, not an error: each says what happened in a toast,
   * and only a carried out decision (or a proposal somebody decided already) takes the row away.
   */
  async decide(item: KnowledgeHeldItem, decision: KnowledgeDecision): Promise<DecisionEnd> {
    if (this.busy.has(item.id)) {
      return 'stay';
    }

    this.busy = new Set([...this.busy, item.id]);

    try {
      const answer = await this.transport.invoke('knowledge_held_decide', { id: item.id, decision, revision: item.revision });

      this.say(item, answer);
      await this.refresh();

      return answer.type === 'approved' || answer.type === 'rejected' || answer.type === 'notFound' ? 'left' : 'stay';
    } catch (error) {
      toasts.push({ title: t(decision === 'approve' ? 'knowledge.held.toast.approveFailed' : 'knowledge.held.toast.rejectFailed'), description: IpcFailure.from(error).message, tone: 'danger' });

      return 'stay';
    } finally {
      this.busy = new Set([...this.busy].filter((id) => id !== item.id));
    }
  }

  private say(item: KnowledgeHeldItem, answer: KnowledgeDecided): void {
    switch (answer.type) {
      case 'approved': {
        const key = answer.status === 'added' ? 'knowledge.held.toast.added' : answer.status === 'updated' ? 'knowledge.held.toast.updated' : 'knowledge.held.toast.unchanged';

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
        toasts.push({ title: t('knowledge.held.toast.changed'), description: item.title, tone: 'warning' });

        return;
      case 'refused':
        toasts.push({ title: t('knowledge.held.toast.refused'), description: answer.message, tone: 'danger' });
    }
  }
}

export const knowledgeHeld = new KnowledgeHeldStore(ipc);
