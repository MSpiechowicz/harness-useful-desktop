import { t } from '$lib/i18n/i18n.svelte';
import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts, type ToastsStore } from '$lib/stores/toasts.svelte';

import { onboarding } from '../onboarding/onboarding-state.svelte';
import { setup } from '../onboarding/setup-state.svelte';

/** The Harness family members that have an official installer the app can run, in the order they are updated. */
const UPDATABLE: readonly EcosystemId[] = ['dashboard', 'memory', 'knowledge', 'code'];

/** The key of the one install Harness Code gets without being installed. */
const CODE_INSTALL_KEY = 'code:install';

interface Dependencies {
  transport: Transport;
  /** The latest reading of the ecosystem, releases included; null while it has not been read. */
  status: () => EcosystemStatus | null;
  /** Whether family updates are wanted; read at every pass so switching a setting off takes effect at once. */
  enabled: () => boolean;
  /** Whether the first-run setup is in the middle of its own installs, which the installers must not race. */
  busy: () => boolean;
  /** Called after a member was updated, so the ecosystem reading and whatever depends on the new binary catch up. */
  updated: (id: EcosystemId) => Promise<void>;
  notifications: Pick<ToastsStore, 'push'>;
}

/**
 * Keeps the installed Harness family members (Harness Dashboard, Harness Memory, Harness Knowledge and Harness Code) current without the user clicking.
 * When the latest reading says an installed member has a newer release, the official installer runs, one member at a
 * time, through the same commands as the first-run setup. The one member it installs without being there is Harness Code, once,
 * for a user who has Harness Memory without it (Harness Memory 3.0 no longer carries the code tools). The backend records that
 * claim before the install starts, so it is not repeated after a failure or after the user removed Harness Code.
 *
 * Each release of a member is tried once per launch, whatever the outcome, so a failing installer produces one toast and
 * not one per reading.
 */
export class FamilyUpdater {
  private readonly attempted = new Set<string>();
  private checking = false;
  private again = false;

  constructor(private readonly deps: Dependencies) {}

  /** Looks at the latest reading and updates what is outdated. Never throws; call it whenever the reading changes. */
  async check(): Promise<void> {
    // A reading that arrives mid-run is not lost: the run looks once more when it ends.
    if (this.checking) {
      this.again = true;

      return;
    }

    this.checking = true;

    try {
      do {
        this.again = false;
        await this.pass();
      } while (this.again);
    } finally {
      this.checking = false;
    }
  }

  private async pass(): Promise<void> {
    for (const id of UPDATABLE) {
      if (!this.deps.enabled() || this.deps.busy()) {
        return;
      }

      const item = this.outdated(id);

      if (item !== null) {
        await this.update(item);
      }
    }

    await this.installMissingCode();
  }

  /** Installs Harness Code when Harness Memory is installed without it, once per launch and once ever (the backend's claim). */
  private async installMissingCode(): Promise<void> {
    if (!this.deps.enabled() || this.deps.busy() || this.attempted.has(CODE_INSTALL_KEY)) {
      return;
    }

    const items = this.deps.status()?.items;
    const memory = items?.find((candidate) => candidate.id === 'memory');
    const code = items?.find((candidate) => candidate.id === 'code');

    if (!memory?.installed || !code || code.installed) {
      return;
    }

    // Whatever the answer, not again in this launch: a refusal stays a refusal, and a failure is told once.
    this.attempted.add(CODE_INSTALL_KEY);

    try {
      if (!(await this.deps.transport.invoke('code_auto_install_claim', undefined))) {
        return;
      }
    } catch {
      return;
    }

    const name = code.name;
    let failure: string | null;

    try {
      const result = await this.install('code');

      failure = result.ok ? null : (result.summary ?? t('ecosystem.autoUpdate.noReason'));
    } catch (error) {
      const reason = IpcFailure.from(error);

      // The user's own install of it is running: it does the job.
      if (isAlreadyRunning(reason)) {
        return;
      }

      failure = reason.message;
    }

    if (failure === null) {
      this.deps.notifications.push({ title: t('ecosystem.autoInstall.done', { name }), tone: 'success' });
      await this.deps.updated('code');
    } else {
      this.deps.notifications.push({ title: t('ecosystem.autoInstall.failed', { name }), description: failure, tone: 'danger' });
    }
  }

  /** The member when it is installed, has a newer release and was not tried for that release yet. */
  private outdated(id: EcosystemId): EcosystemItem | null {
    const item = this.deps.status()?.items.find((candidate) => candidate.id === id);

    if (!item || !item.installed || !item.updateAvailable || !item.latestVersion) {
      return null;
    }

    return this.attempted.has(attemptKey(item)) ? null : item;
  }

  private async update(item: EcosystemItem): Promise<void> {
    const key = attemptKey(item);
    const name = item.name;

    let failure: string | null;
    let version = item.latestVersion;

    try {
      const result = await this.install(item.id);

      failure = result.ok ? null : (result.summary ?? t('ecosystem.autoUpdate.noReason'));
      version = result.version ?? version;
    } catch (error) {
      const reason = IpcFailure.from(error);

      // Another install of this member is running (the user's own): not a failure, and the next reading tries again.
      if (isAlreadyRunning(reason)) {
        return;
      }

      failure = reason.message;
    }

    this.attempted.add(key);

    if (failure === null) {
      this.deps.notifications.push({ title: t('ecosystem.autoUpdate.done', { name, version: version ?? '' }), tone: 'success' });
      await this.deps.updated(item.id);
    } else {
      this.deps.notifications.push({ title: t('ecosystem.autoUpdate.failed', { name }), description: failure, tone: 'danger' });
    }
  }

  private install(id: EcosystemId) {
    const args = { confirmed: true, onLog: () => {} };

    switch (id) {
      case 'dashboard':
        return this.deps.transport.invoke('setup_install_dashboard', args);
      case 'memory':
        return this.deps.transport.invoke('setup_install_memory', args);
      case 'knowledge':
        return this.deps.transport.invoke('setup_install_knowledge', args);
      case 'code':
        return this.deps.transport.invoke('setup_install_code', args);
      case 'skills':
      case 'sidebar':
        // Not installed by an installer the app runs, so they are not in UPDATABLE.
        throw new Error(`${id} has no installer the app runs`);
    }
  }
}

function attemptKey(item: EcosystemItem): string {
  return `${item.id}@${item.latestVersion ?? ''}`;
}

/** The backend answers `invalidInput` ("<binary> is already being installed") while an install of that member runs. */
function isAlreadyRunning(failure: IpcFailure): boolean {
  return failure.code === 'invalidInput' && /already being installed/i.test(failure.message);
}

export const familyUpdater = new FamilyUpdater({
  transport: ipc,
  status: () => ecosystem.status,
  enabled: () => (prefs.value?.autoUpdateExtensions ?? false) && (prefs.value?.checkCompanionUpdates ?? true),
  busy: () => onboarding.visible || setup.running,
  updated: async (id) => {
    await ecosystem.load(true);

    // The installer re-registers the memory, knowledge or code server (the backend owns that part of the preferences).
    if (id === 'memory' || id === 'knowledge' || id === 'code') {
      await prefs.syncServerManaged();
    }
  },
  notifications: toasts,
});
