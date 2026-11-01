import type { KnowledgeStatus } from '$lib/ipc/bindings/KnowledgeStatus';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';

/** Whether Harness Knowledge is installed and current, and what it can do. Unknown stays null, and nothing is shown about it. */
export class KnowledgeStatusStore {
  value = $state.raw<KnowledgeStatus | null>(null);

  constructor(private readonly transport: Transport) {}

  /** The answer of the server is `notInstalled` or `outdated`: the row that installs or updates it shows. */
  get installable(): 'install' | 'update' | null {
    switch (this.value?.availability.type) {
      case 'notInstalled':
        return 'install';
      case 'outdated':
        return 'update';
      default:
        return null;
    }
  }

  async load(): Promise<void> {
    try {
      this.value = await this.transport.invoke('knowledge_status', undefined);
    } catch {
      // The next look repairs it, and until then nothing is claimed.
    }
  }
}

export const knowledgeStatus = new KnowledgeStatusStore(ipc);
