import type { KnowledgeStats } from '$lib/ipc/bindings/KnowledgeStats';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';

/** What the database of Harness Knowledge says about itself that the document actions need. Unknown stays null, and nothing is claimed about it. */
export class KnowledgeStatsStore {
  value = $state.raw<KnowledgeStats | null>(null);

  constructor(private readonly transport: Transport) {}

  /** Whether the user may change the remote place: false for a read-only role, null while not known (actions stay enabled then). */
  get remoteWritable(): boolean | null {
    return this.value?.remoteWritable ?? null;
  }

  async load(): Promise<void> {
    try {
      this.value = await this.transport.invoke('knowledge_stats', undefined);
    } catch {
      // The next look repairs it, and until then what was known stays.
    }
  }

  /** Forgets what was read, e.g. for a test. */
  reset(): void {
    this.value = null;
  }
}

export const knowledgeStats = new KnowledgeStatsStore(ipc);
