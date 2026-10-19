import { t } from '$lib/i18n/i18n.svelte';
import type { MessageKey } from '$lib/i18n/locales/en';
import { IpcFailure } from '$lib/ipc/transport';

import { toasts } from './toasts.svelte';

/** What to say when a change fails, and how to read the list again after it. */
export interface DocOutcome {
  /** The toast title of a failure: "Could not delete the hook". The reason the backend gave is its description. */
  failed: MessageKey;
  /** The toast of a conflict: someone (an agent) rewrote the file since the user looked. */
  conflict: MessageKey;
  conflictHint: MessageKey;
  /** Reads the list again, whatever happened. */
  refresh: () => Promise<unknown>;
}

/** The documents (hooks, routines) a switch or a button is working on, by row key, so that a second click waits. */
export class DocBusy {
  keys = $state<ReadonlySet<string>>(new Set());

  /** Runs one change on a document: marks it busy, then reads the list again whatever happened. A failure is a toast, a conflict one that says the file changed. */
  async run(key: string, change: () => Promise<unknown>, outcome: DocOutcome): Promise<boolean> {
    this.keys = new Set([...this.keys, key]);

    try {
      await change();

      return true;
    } catch (error) {
      const cause = IpcFailure.from(error);

      if (cause.code === 'conflict') {
        toasts.push({ title: t(outcome.conflict), description: t(outcome.conflictHint), tone: 'warning' });
      } else {
        toasts.push({ title: t(outcome.failed), description: cause.message, tone: 'danger' });
      }

      return false;
    } finally {
      await outcome.refresh();
      this.keys = new Set([...this.keys].filter((other) => other !== key));
    }
  }

  reset(): void {
    this.keys = new Set();
  }
}
