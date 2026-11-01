import { t } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import { idKey, type KnowledgeId } from '$lib/ipc/knowledge-id';
import type { Transport } from '$lib/ipc/transport';
import { readFailureText } from '$lib/utils/knowledge-failure';

/** Where the text of one chunk stands. */
export type ChunkState = { status: 'loading' } | { status: 'ready'; text: string } | { status: 'error'; message: string };

/**
 * The texts of the chunks of one document, read when first asked for and kept while the panel shows the document, so an outline row
 * and a search match that open the same chunk read it once.
 */
export class ChunkTexts {
  private states = $state.raw<Record<string, ChunkState>>({});

  constructor(
    readonly docId: KnowledgeId,
    private readonly transport: Transport = ipc,
  ) {}

  stateOf(chunkId: KnowledgeId): ChunkState | undefined {
    return this.states[idKey(chunkId)];
  }

  /** Reads the chunk unless it is read or being read. A failed one is read again. */
  async load(chunkId: KnowledgeId): Promise<void> {
    const known = this.states[idKey(chunkId)];

    if (known !== undefined && known.status !== 'error') {
      return;
    }

    this.set(chunkId, { status: 'loading' });

    try {
      const detail = await this.transport.invoke('knowledge_doc_get', { id: this.docId, chunkId });

      this.set(chunkId, detail.chunk === null ? { status: 'error', message: t('knowledge.chunk.failed') } : { status: 'ready', text: detail.chunk.text });
    } catch (error) {
      this.set(chunkId, { status: 'error', message: readFailureText(error) });
    }
  }

  private set(chunkId: KnowledgeId, state: ChunkState): void {
    this.states = { ...this.states, [idKey(chunkId)]: state };
  }
}
