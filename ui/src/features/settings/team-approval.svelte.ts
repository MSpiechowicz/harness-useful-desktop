import { t } from '$lib/i18n/i18n.svelte';
import type { KnowledgeTeamApproval } from '$lib/ipc/bindings/KnowledgeTeamApproval';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { writeFailureText } from '$lib/utils/knowledge-failure';

/** What a read of the setting found when it could not answer. */
export type ApprovalReadFailure = 'unsupported' | 'unreachable';

/**
 * The team approval setting. It lives in the team database, so it is read from there and a change applies at once, not with the form's Save. The switch
 * shows what the database last said: a refused change leaves it where it was and says why in a toast.
 */
export class TeamApprovalStore {
  value = $state.raw<KnowledgeTeamApproval | null>(null);
  /** Why the last read gave no answer, or null. */
  readFailure = $state<ApprovalReadFailure | null>(null);
  /** A change is being saved. */
  pending = $state(false);

  /** Only the newest read may apply its answer. */
  private seq = 0;

  constructor(private readonly transport: Transport) {}

  /** Reads the setting again. A read that finishes while a change is saving is dropped: the change's answer is newer. */
  async load(): Promise<void> {
    this.seq += 1;
    const mine = this.seq;

    try {
      const value = await this.transport.invoke('knowledge_approval_get', undefined);

      if (mine === this.seq && !this.pending) {
        this.value = value;
        this.readFailure = null;
      }
    } catch (error) {
      if (mine === this.seq && !this.pending) {
        this.readFailure = failureOf(error);
      }
    }
  }

  /** Turns team approval on or off for the team. Refusals and failures are said in a toast and leave the switch where it was. */
  async set(on: boolean): Promise<void> {
    if (this.pending || !this.value) {
      return;
    }

    this.pending = true;
    this.seq += 1;

    try {
      const outcome = await this.transport.invoke('knowledge_approval_set', { on });

      if (outcome.type === 'done') {
        this.value = outcome.value;
        this.readFailure = null;
        toasts.push({ title: on ? t('settings.knowledgeApproval.on') : t('settings.knowledgeApproval.off'), description: on ? t('settings.knowledgeApproval.on.body') : t('settings.knowledgeApproval.off.body'), tone: 'success' });

        return;
      }

      // The role may not write: the row says so from now on, and the toast gives the server's reason.
      if (outcome.code === 'readOnly') {
        this.value = { ...this.value, writable: false };
      }

      // No git name: the row says so from now on, as the review does.
      if (outcome.code === 'noName') {
        knowledgeViewer.learnNoName();
      }

      toasts.push({ title: t('settings.knowledgeApproval.failed'), description: outcome.message, tone: 'danger' });
    } catch (error) {
      toasts.push({ title: t('settings.knowledgeApproval.failed'), description: writeFailureText(error), tone: 'danger' });
    } finally {
      this.pending = false;
    }
  }

  /** Forgets what was read, e.g. for a test. */
  reset(): void {
    this.seq += 1;
    this.value = null;
    this.readFailure = null;
    this.pending = false;
  }
}

function failureOf(error: unknown): ApprovalReadFailure {
  // Anything but a server that predates the setting is a database that gave no answer.
  return IpcFailure.from(error).code === 'unsupported' ? 'unsupported' : 'unreachable';
}

export const teamApproval = new TeamApprovalStore(ipc);
