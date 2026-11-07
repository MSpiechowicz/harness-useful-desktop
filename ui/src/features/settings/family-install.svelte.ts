import { t, type MessageKey } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import { IpcFailure } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { prefs } from '$lib/stores/prefs.svelte';

import { CompanionInstaller, type InstallOutcome } from '../onboarding/companion-installer.svelte';
import { companionPlan, type Companion, type Plan } from '../onboarding/setup-model';

export type InstallPhase = 'idle' | 'reading' | 'running' | 'failed';

/** The members of the Harness family that are installed or updated from where they are missed, not only in the onboarding. */
export type InstallMember = 'knowledge' | 'code';

/** The words around the install of one member, as message keys. */
export interface InstallCopy {
  missing: { title: MessageKey; description: MessageKey };
  outdated: { title: MessageKey; description: MessageKey };
  install: MessageKey;
  update: MessageKey;
  installing: MessageKey;
  updating: MessageKey;
  ecosystem: MessageKey;
  readFailed: MessageKey;
  confirm: { install: MessageKey; update: MessageKey; body: MessageKey };
}

export const INSTALL_COPY: Record<InstallMember, InstallCopy> = {
  knowledge: {
    missing: { title: 'knowledge.settings.install.missing.title', description: 'knowledge.settings.install.missing.description' },
    outdated: { title: 'knowledge.settings.install.outdated.title', description: 'knowledge.settings.install.outdated.description' },
    install: 'knowledge.settings.install.install',
    update: 'knowledge.settings.install.update',
    installing: 'knowledge.settings.install.installing',
    updating: 'knowledge.settings.install.updating',
    ecosystem: 'knowledge.settings.install.ecosystem',
    readFailed: 'knowledge.settings.install.readFailed',
    confirm: { install: 'knowledge.settings.install.confirm.install', update: 'knowledge.settings.install.confirm.update', body: 'knowledge.settings.install.confirm.body' },
  },
  code: {
    missing: { title: 'settings.code.install.missing.title', description: 'settings.code.install.missing.description' },
    outdated: { title: 'settings.code.install.outdated.title', description: 'settings.code.install.outdated.description' },
    install: 'settings.code.install.install',
    update: 'settings.code.install.update',
    installing: 'settings.code.install.installing',
    updating: 'settings.code.install.updating',
    ecosystem: 'settings.code.install.ecosystem',
    readFailed: 'settings.code.install.readFailed',
    confirm: { install: 'settings.code.install.confirm.install', update: 'settings.code.install.confirm.update', body: 'settings.code.install.confirm.body' },
  },
};

/**
 * Installing or updating a Harness family member: read what the installer would do, ask,
 * run it, then look at the installed server again. Nothing runs before the user confirmed the plan.
 */
export class CompanionInstallFlow {
  phase = $state<InstallPhase>('idle');
  plan = $state.raw<Plan | null>(null);
  confirming = $state(false);
  failure = $state('');
  /** How the last run ended. */
  outcome: InstallOutcome | null = null;

  private readonly installer: CompanionInstaller;

  constructor(
    readonly companion: Companion,
    /** An installed but too old member is updated, a missing one installed. */
    readonly update: () => boolean,
    /** Looks at the installed member again once the installer ran. */
    private readonly onfinished: () => Promise<void>,
    private readonly readFailed: MessageKey,
  ) {
    this.installer = new CompanionInstaller(companion, ipc);
  }

  get busy(): boolean {
    return this.phase === 'reading' || this.phase === 'running';
  }

  /** Reads what the installer would do, then asks. */
  async review(): Promise<void> {
    this.phase = 'reading';

    try {
      this.plan = companionPlan(this.companion, (await ipc.invoke('setup_status', undefined))[this.companion]);
      this.phase = 'idle';
      this.confirming = true;
    } catch (error) {
      this.giveUp(t(this.readFailed, { reason: IpcFailure.from(error).message }));
    }
  }

  async run(): Promise<void> {
    this.phase = 'running';

    const outcome = await this.installer.run();

    this.outcome = outcome;

    if (!outcome.ok) {
      this.giveUp(this.installer.result?.timedOut ? t('onboarding.setup.companion.timedOut') : t('onboarding.setup.companion.failed', { reason: outcome.failure || t('onboarding.setup.companion.noReason') }));

      return;
    }

    // The Ecosystem page and the preferences follow the new binary, like after any other install.
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

/** The flow where a member is missed (the Knowledge view, the settings), with the words of that member. */
export class FamilyInstallFlow extends CompanionInstallFlow {
  constructor(
    readonly member: InstallMember,
    update: () => boolean,
    onfinished: () => Promise<void>,
  ) {
    super(member, update, onfinished, INSTALL_COPY[member].readFailed);
  }

  get copy(): InstallCopy {
    return INSTALL_COPY[this.member];
  }
}
