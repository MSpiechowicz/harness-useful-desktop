import { SvelteMap } from 'svelte/reactivity';

import { ipc } from '$lib/ipc/client';
import type { KnowledgeDocument } from '$lib/ipc/bindings/KnowledgeDocument';
import { idFromKey } from '$lib/ipc/knowledge-id';

/** How long a document that could not be read is left alone before the next try. */
const RETRY_MS = 30_000;

/**
 * The documents the chat cards show, read once from Harness Knowledge by the id a reply names. A search reply tells a title and
 * a section but not the format, the place or the repositories, so a card asks for them here and shows them once they are known.
 */
class DocLookup {
  private readonly known = new SvelteMap<string, KnowledgeDocument>();
  private readonly tried = new Map<string, number>();
  private readonly reading = new Set<string>();

  /** The document `ref` (`12`, `local:12`) once it was read; undefined until then and when it cannot be read. */
  get(ref: string): KnowledgeDocument | undefined {
    return this.known.get(ref);
  }

  /** Reads document `ref` unless it is known, being read, or failed a moment ago. A failure (knowledge off, a deleted document) leaves it unknown. */
  async ensure(ref: string): Promise<void> {
    const failed = this.tried.get(ref);
    if (this.known.has(ref) || this.reading.has(ref) || (failed !== undefined && Date.now() - failed < RETRY_MS)) {
      return;
    }

    this.reading.add(ref);

    try {
      const { document } = await ipc.invoke('knowledge_doc_get', { id: idFromKey(ref), chunkId: null });
      this.known.set(ref, document);
    } catch {
      this.tried.set(ref, Date.now());
    } finally {
      this.reading.delete(ref);
    }
  }
}

export const docLookup = new DocLookup();
