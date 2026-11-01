import { t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeDeleted } from '$lib/ipc/bindings/KnowledgeDeleted';
import type { KnowledgeDetail } from '$lib/ipc/bindings/KnowledgeDetail';
import type { KnowledgeLink } from '$lib/ipc/bindings/KnowledgeLink';
import type { KnowledgePage } from '$lib/ipc/bindings/KnowledgePage';
import type { KnowledgePlace } from '$lib/ipc/bindings/KnowledgePlace';
import type { KnowledgeReposPage } from '$lib/ipc/bindings/KnowledgeReposPage';
import type { KnowledgeScope } from '$lib/ipc/bindings/KnowledgeScope';
import type { KnowledgeSort } from '$lib/ipc/bindings/KnowledgeSort';
import type { SortOrder } from '$lib/ipc/bindings/SortOrder';
import { ipc } from '$lib/ipc/client';
import { idFromKey, type KnowledgeId } from '$lib/ipc/knowledge-id';
import type { Transport } from '$lib/ipc/transport';
import { readFailureText } from '$lib/utils/knowledge-failure';

import { reachKey } from './memory-reach';
import { toasts } from './toasts.svelte';
import { ViewLoader } from './view-loader.svelte';

/** How many documents one page of the table holds. */
export const KNOWLEDGE_PAGE_SIZE = 50;

/** The most documents one list call returns: the size of the batches "delete all" walks the database in. */
const BATCH = 200;

/** What the table asks the server for: one page of the documents that answer the search, the repository and the format, in this order. */
export interface DocsRequest {
  query: string;
  /** A Scope filter choice: `all`, `global` or `repo:<key>`. */
  scope: string;
  format: string | null;
  /** Narrows the table to the documents kept in one place, when both are in use. */
  place: KnowledgePlace | null;
  sort: KnowledgeSort;
  order: SortOrder;
  page: number;
}

/** The request as the loader's key: the loader reads again whenever any of it changes, and drops answers to older ones. */
export function docsKey(request: DocsRequest): string {
  return JSON.stringify(request);
}

/** What a Scope filter choice asks of the server. A project without a repository key names no repository, so it reads every document. */
export function scopeOf(choice: string): KnowledgeScope {
  if (choice === 'global') {
    return { kind: 'global' };
  }

  const key = reachKey(choice);

  return key === null ? { kind: 'all' } : { kind: 'repo', key };
}

/** The documents the table shows, one document read for the panel, and the repositories that have documents, kept live by `follow`. */
export class KnowledgeDocsStore {
  readonly list: ViewLoader<KnowledgePage, string>;
  readonly detail: ViewLoader<KnowledgeDetail, string>;
  readonly repos: ViewLoader<KnowledgeReposPage, string>;

  constructor(private readonly transport: Transport) {
    this.list = new ViewLoader((key) => {
      const { query, scope, format, place, sort, order, page } = JSON.parse(key) as DocsRequest;
      const search = query.trim();

      return transport.invoke('knowledge_docs_list', {
        filter: { scope: scopeOf(scope), format, place, sort, order },
        offset: page * KNOWLEDGE_PAGE_SIZE,
        limit: KNOWLEDGE_PAGE_SIZE,
        query: search === '' ? null : search,
      });
    });
    this.detail = new ViewLoader((key) => transport.invoke('knowledge_doc_get', { id: idFromKey(key), chunkId: null }), { goneIsFinal: true });
    this.repos = new ViewLoader(() => transport.invoke('knowledge_repos_list', undefined));
  }

  /** Reads everything on screen again without a loading state: the database changed (a pulse), or this view changed it. */
  refresh(): Promise<unknown> {
    return Promise.all([this.list.refresh({ quiet: true }), this.detail.refresh({ quiet: true }), this.repos.refresh({ quiet: true })]);
  }

  /** Renames a document and answers the title as stored; null (with a toast) when the server refused. */
  async rename(id: KnowledgeId, title: string): Promise<string | null> {
    try {
      const stored = await this.transport.invoke('knowledge_doc_rename', { id, title });

      await this.refresh();

      return stored;
    } catch (error) {
      toasts.push({ title: t('knowledge.toast.renameFailed'), description: readFailureText(error), tone: 'danger' });

      return null;
    }
  }

  /** Replaces what a document links to and answers the links as stored; null (with a toast) when the server refused. */
  async setLinks(id: KnowledgeId, links: KnowledgeLink[]): Promise<KnowledgeLink[] | null> {
    try {
      const stored = await this.transport.invoke('knowledge_doc_links_set', { id, links });

      await this.refresh();

      return stored;
    } catch (error) {
      toasts.push({ title: t('knowledge.toast.linksFailed'), description: readFailureText(error), tone: 'danger' });

      return null;
    }
  }

  /** Deletes documents, then reads again so their rows are gone (or back, when the call failed). Answers how many went; null (with a toast) on failure. */
  async remove(ids: KnowledgeId[]): Promise<KnowledgeDeleted | null> {
    try {
      const deleted = await this.transport.invoke('knowledge_docs_delete', { ids });

      await this.refresh();

      return deleted;
    } catch (error) {
      toasts.push({ title: t('knowledge.toast.deleteFailed'), description: readFailureText(error), tone: 'danger' });
      await this.refresh();

      return null;
    }
  }

  /**
   * Deletes every document, a batch at a time, whatever the table is narrowed to. Rejects with the server's reason, and when a page
   * comes back from a server that is not available (that is no empty database). Answers how many went and how many there were at
   * the start, so a caller can tell when some remain.
   */
  async removeAll(): Promise<{ deleted: number; total: number }> {
    let deleted = 0;
    let total: number | null = null;

    try {
      for (;;) {
        const page = await this.transport.invoke('knowledge_docs_list', { filter: null, offset: 0, limit: BATCH, query: null });

        if (page.availability.type !== 'available') {
          throw new Error(this.why(page.availability));
        }

        total ??= page.total;

        if (page.items.length === 0) {
          break;
        }

        const gone = (await this.transport.invoke('knowledge_docs_delete', { ids: page.items.map((doc) => doc.id) })).deleted;

        // A batch that took nothing would be read again for ever.
        if (gone === 0) {
          break;
        }

        deleted += gone;
      }
    } finally {
      // A refresh that fails must not hide why the deletion stopped.
      await this.refresh().catch(() => undefined);
    }

    return { deleted, total: total ?? 0 };
  }

  /** What a page that is not available says, for the error of a delete that could not go on. */
  private why(availability: KnowledgePage['availability']): string {
    return 'message' in availability && availability.message !== '' ? availability.message : t('knowledge.deleteAll.unavailable');
  }
}

export const knowledgeDocs = new KnowledgeDocsStore(ipc);
