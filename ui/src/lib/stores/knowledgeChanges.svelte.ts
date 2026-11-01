import type { KnowledgePulse } from '$lib/ipc/bindings/KnowledgePulse';
import { ipc } from '$lib/ipc/client';
import type { Transport } from '$lib/ipc/transport';

/**
 * What the database of Harness Knowledge says about itself, live: the document count, a generation that moves whenever it changes
 * (the view reads its list again then) and the proposals waiting for review, which the rail shows as a badge. The backend sends it now and on
 * every change, and nothing while project knowledge is off or the server does not answer, so unknown stays null.
 */
export class KnowledgeChangesStore {
  pulse = $state.raw<KnowledgePulse | null>(null);

  constructor(private readonly transport: Transport) {}

  /** How many proposals wait for review. Zero while it is not known, so no badge shows. */
  get held(): number {
    return this.pulse?.held ?? 0;
  }

  /** Follows the database from now on, replacing the previous listener. Returns the function that stops listening. */
  start(): () => void {
    let listening = true;

    this.transport
      .invoke('knowledge_changes_listen', {
        onEvent: (pulse) => {
          if (listening) {
            this.pulse = pulse;
          }
        },
      })
      .catch(() => {
        // No server to follow: the rail shows no badge.
      });

    return () => {
      listening = false;
    };
  }

  /** Forgets what was read, e.g. for a test. */
  reset(): void {
    this.pulse = null;
  }
}

export const knowledgeChanges = new KnowledgeChangesStore(ipc);
