import { t } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { CompanionInstaller } from '../onboarding/companion-installer.svelte';
import { companionPlan, type Plan } from '../onboarding/setup-model';

export type InstallPhase = 'idle' | 'reading' | 'running' | 'failed';

/**
 * Installing or updating Harness Knowledge from where it is missed (the Knowledge view, the settings): read what the installer would do, ask,
 * run it, then look at the installed server again. Nothing runs before the user confirmed the plan.
 */
export class KnowledgeInstallFlow {
  phase = $state<InstallPhase>('idle');
  plan = $state.raw<Plan | null>(null);
  confirming = $state(false);
  failure = $state('');

  private readonly installer = new CompanionInstaller('knowledge', ipc);

  constructor(
    /** An installed but too old Harness Knowledge is updated, a missing one installed. */
    readonly update: () => boolean,
    /** Looks at the installed Harness Knowledge again once the installer ran. */
    private readonly onfinished: () => Promise<void>,
  ) {}

  get busy(): boolean {
    return this.phase === 'reading' || this.phase === 'running';
  }

  /** Reads what the installer would do, then asks. */
  async review(): Promise<void> {
    this.phase = 'reading';

    try {
      this.plan = companionPlan('knowledge', (await ipc.invoke('setup_status', undefined)).knowledge);
      this.phase = 'idle';
      this.confirming = true;
    } catch (error) {
      this.giveUp(t('knowledge.settings.install.readFailed', { reason: IpcFailure.from(error).message }));
    }
  }

  async run(): Promise<void> {
    this.phase = 'running';

    const outcome = await this.installer.run();

    if (!outcome.ok) {
      this.giveUp(this.installer.result?.timedOut ? t('onboarding.setup.companion.timedOut') : t('onboarding.setup.companion.failed', { reason: outcome.failure || t('onboarding.setup.companion.noReason') }));

      return;
    }

    // The Ecosystem page and the knowledge preferences follow the new binary, like after any other install.
    void ecosystem.load(true);
    void prefs.syncServerManaged().catch(() => {});

    await this.onfinished();
    this.phase = 'idle';
  }

  private giveUp(reason: string): void {
    this.failure = reason;
    this.phase = 'failed';
  }
}
