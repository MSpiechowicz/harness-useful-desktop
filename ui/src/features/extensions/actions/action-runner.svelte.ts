import type { CliResult } from '$lib/ipc/bindings/CliResult';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import { ipc } from '$lib/ipc/client';
import { t } from '$lib/i18n/i18n.svelte';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { agentSessions } from '$lib/stores/agentSession.svelte';
import { toasts } from '$lib/stores/toasts.svelte';
import { extensionsView } from '$lib/stores/views.svelte';

import { failureLine, outcomeIn, type ActionRequest, type Outcome } from './action-model';

export type ActionPhase = 'idle' | 'previewing' | 'ready' | 'running' | 'done';

/**
 * One extension mutation at a time, always in the same order: preview (a dry run, which never
 * changes anything), the user's confirmation, the real run, then a re-read of the lists to see
 * whether the change really happened.
 */
export class ActionRunner {
  phase = $state<ActionPhase>('idle');
  request = $state.raw<ActionRequest | null>(null);
  /** The dry run's result; `exitCode` null means the CLI has no preview for this action. */
  preview = $state.raw<CliResult | null>(null);
  result = $state.raw<CliResult | null>(null);
  /** The reason a request could not be made at all (refused target, harness missing). */
  error = $state<string | null>(null);
  outcome = $state<Outcome | null>(null);

  private token = 0;

  constructor(
    private readonly transport: Transport,
    private readonly reload: () => Promise<ExtensionsView | null>,
    /** Called after a run that went through, with the request: open chats may need to pick the change up. */
    private readonly applied: (request: ActionRequest) => Promise<void> = async () => {},
  ) {}

  /** The failure line of the real run, or null while it has not failed. */
  get failure(): string | null {
    return this.result ? failureLine(this.result) : null;
  }

  /** Whether confirming makes sense: the preview did not already show a failure. */
  get confirmable(): boolean {
    return this.phase === 'ready' && this.preview !== null && failureLine(this.preview) === null;
  }

  /** Opens the confirmation and runs the preview. */
  async begin(request: ActionRequest): Promise<void> {
    this.token += 1;
    const mine = this.token;

    this.request = request;
    this.preview = null;
    this.result = null;
    this.error = null;
    this.outcome = null;
    this.phase = 'previewing';

    try {
      const preview = await this.call(request, true);

      if (mine === this.token) {
        this.preview = preview;
        this.phase = 'ready';
      }
    } catch (error) {
      if (mine === this.token) {
        this.error = IpcFailure.from(error).message;
        this.phase = 'ready';
      }
    }
  }

  /** Runs the confirmed action, then re-reads the lists. */
  async confirm(): Promise<void> {
    const request = this.request;
    if (!request || !this.confirmable) {
      return;
    }

    const mine = this.token;
    this.phase = 'running';

    try {
      this.result = await this.call(request, false);
    } catch (error) {
      this.error = IpcFailure.from(error).message;
    }

    // Even a failed run may have changed something: look again either way.
    const view = await this.reload();
    if (this.result && this.failure === null) {
      await this.applied(request);
    }

    if (mine === this.token) {
      this.outcome = this.result && this.failure === null && view ? outcomeIn(view, request.action) : null;
      this.phase = 'done';
    }
  }

  /** Closes the dialog; a run in progress cannot be abandoned. */
  close(): void {
    if (this.phase === 'running') {
      return;
    }

    this.token += 1;
    this.phase = 'idle';
    this.request = null;
  }

  private call(request: ActionRequest, dryRun: boolean): Promise<CliResult> {
    return this.transport.invoke('extension_action', { harness: request.harness, action: request.action, dryRun, project: request.project });
  }
}

/** What changes the plugins an agent has loaded; a chat that started before it has to be restarted to see it. */
const PLUGIN_CHANGES = new Set(['pluginInstall', 'pluginUpgrade', 'pluginUninstall', 'pluginEnable', 'pluginDisable']);

export const actionRunner = new ActionRunner(
  ipc,
  async () => {
    await extensionsView.refresh();

    return extensionsView.data;
  },
  async (request) => {
    if (PLUGIN_CHANGES.has(request.action.type) && (await agentSessions.reloadIdle(request.harness)) > 0) {
      toasts.push({ title: t('extensions.toast.chatsReloaded'), tone: 'info' });
    }
  },
);
