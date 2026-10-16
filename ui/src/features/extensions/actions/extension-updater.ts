import { t } from '$lib/i18n/i18n.svelte';
import type { CliResult } from '$lib/ipc/bindings/CliResult';
import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { harness } from '$lib/stores/harness.svelte';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts, type ToastsStore } from '$lib/stores/toasts.svelte';
import { extensionsView } from '$lib/stores/views.svelte';

import { failureLine } from './action-model';
import { actionRunner, reloadChatsAfterPluginChange } from './action-runner.svelte';

/** How often the installed harnesses are looked at. */
export const EXTENSION_UPDATE_INTERVAL_MS = 10 * 60_000;
/** The first look, a little after start so it never competes with the app opening. */
export const EXTENSION_UPDATE_FIRST_DELAY_MS = 30_000;
/** PI cannot say whether a package has an update, so asking costs a network round trip per package; do it less often. */
export const PACKAGE_UPDATE_INTERVAL_MS = 60 * 60_000;

const NAMES_SHOWN = 3;

interface Dependencies {
  transport: Transport;
  /** The harnesses detected on this machine. */
  installed: () => HarnessSummary[];
  /** Whether the setting is on; read at every look so switching it off takes effect at once. */
  enabled: () => boolean;
  /** Whether the user is in the middle of an extension action of their own; the two must not run the CLI together. */
  busy: () => boolean;
  /** Called after plugins changed, so open chats of the harness pick the change up. */
  upgraded: (harness: HarnessId) => Promise<void>;
  notifications: Pick<ToastsStore, 'push'>;
  now?: () => number;
}

/**
 * Keeps the plugins, marketplace catalogs and packages of every installed harness current without the user doing
 * anything. Each look refreshes the catalogs, installs the plugin upgrades they reveal (announcing the batch in a
 * toast first) and updates packages. Goes through the same `extension_action` command as the Extensions buttons.
 *
 * A batch is tried once, whatever the outcome, and not again until what is outdated changes, so a plugin that stays
 * outdated does not produce a toast every ten minutes.
 */
export class ExtensionUpdater {
  /** Plugin batches (harness plus ids and versions) already tried, and the package failures already announced. */
  private readonly attempted = new Set<string>();
  private lastPackageRun = new Map<HarnessId, number>();
  private checking = false;

  constructor(private readonly deps: Dependencies) {}

  /** Starts the schedule; call the returned function to stop it. */
  start(): () => void {
    let timer: ReturnType<typeof setTimeout> | null = null;
    let stopped = false;

    const schedule = (delay: number): void => {
      timer = setTimeout(() => {
        void this.check().finally(() => {
          if (!stopped) {
            schedule(EXTENSION_UPDATE_INTERVAL_MS);
          }
        });
      }, delay);
    };

    schedule(EXTENSION_UPDATE_FIRST_DELAY_MS);

    return () => {
      stopped = true;
      if (timer !== null) {
        clearTimeout(timer);
      }
    };
  }

  /** One look at every installed harness. Never throws; a look that finds the user busy does nothing. */
  async check(): Promise<void> {
    if (this.checking || !this.deps.enabled() || this.deps.busy()) {
      return;
    }

    this.checking = true;

    try {
      for (const summary of this.deps.installed()) {
        if (!this.deps.enabled() || this.deps.busy()) {
          break;
        }

        await this.checkHarness(summary);
      }
    } finally {
      this.checking = false;
    }
  }

  private async checkHarness({ info, capabilities }: HarnessSummary): Promise<void> {
    // Updates run through the harness CLI; a harness without CLI actions has none to run.
    if (!capabilities.cliActions) {
      return;
    }

    if (capabilities.marketplaces) {
      await this.refreshMarketplaces(info.id);
    }

    if (capabilities.plugins) {
      await this.upgradePlugins(info.id);
    }

    if (capabilities.packages) {
      await this.updatePackages(info.id);
    }
  }

  /**
   * Catalogs only learn about new versions when they are refreshed. Each one is refreshed by name: a bare
   * `marketplace update` stops at the first catalog that fails (renamed, unreachable) and leaves the later ones stale,
   * so their plugin updates would never be seen. A failure is not worth a message: the plugins are judged by what the
   * catalogs say now.
   */
  private async refreshMarketplaces(harness: HarnessId): Promise<void> {
    let names: string[];

    try {
      names = (await this.view(harness)).marketplaces.map((marketplace) => marketplace.name);
    } catch {
      return;
    }

    for (const name of names) {
      if (!this.deps.enabled() || this.deps.busy()) {
        return;
      }

      await this.run(harness, { type: 'marketplaceUpdate', name }).catch(() => null);
    }
  }

  private view(harness: HarnessId): Promise<ExtensionsView> {
    return this.deps.transport.invoke('extensions_view', { harness, project: null });
  }

  private async upgradePlugins(harness: HarnessId): Promise<void> {
    let view: ExtensionsView;

    try {
      view = await this.view(harness);
    } catch {
      return;
    }

    const outdated = view.plugins.filter((plugin) => plugin.updateAvailable);
    const batch = `${harness}:${outdated
      .map((plugin) => `${plugin.id}@${plugin.latestVersion ?? ''}`)
      .sort()
      .join(',')}`;

    if (outdated.length === 0 || this.attempted.has(batch) || !this.deps.enabled() || this.deps.busy()) {
      return;
    }

    this.deps.notifications.push({
      title: t('extensions.autoUpdate.installing'),
      description: namesOf(outdated.map((plugin) => plugin.name)),
      tone: 'info',
    });

    const failure = await this.run(harness, {
      type: 'pluginUpgrade',
      name: null,
    }).then(failureLine, (error: unknown) => IpcFailure.from(error).message);

    // A batch is tried once: if it is still outdated afterwards, trying again would only repeat the toasts.
    this.attempted.add(batch);

    if (failure === null) {
      this.deps.notifications.push({
        title: t('extensions.autoUpdate.done'),
        description: namesOf(outdated.map((plugin) => plugin.name)),
        tone: 'success',
      });
    } else {
      this.deps.notifications.push({
        title: t('extensions.autoUpdate.failed'),
        description: failure,
        tone: 'warning',
      });
    }

    // Even a failed run may have upgraded some plugins.
    await this.deps.upgraded(harness);
  }

  private async updatePackages(harness: HarnessId): Promise<void> {
    const last = this.lastPackageRun.get(harness);
    const now = (this.deps.now ?? Date.now)();

    if (last !== undefined && now - last < PACKAGE_UPDATE_INTERVAL_MS) {
      return;
    }

    this.lastPackageRun.set(harness, now);

    const failure = await this.run(harness, {
      type: 'packageUpdate',
      source: null,
    }).then(failureLine, (error: unknown) => IpcFailure.from(error).message);
    const key = `${harness}:packages:${failure ?? ''}`;

    // PI does not say what it updated, so a success is silent; a failure is announced once per distinct message.
    if (failure !== null && !this.attempted.has(key)) {
      this.attempted.add(key);
      this.deps.notifications.push({
        title: t('extensions.autoUpdate.packagesFailed'),
        description: failure,
        tone: 'warning',
      });
    }
  }

  private run(harness: HarnessId, action: ExtensionAction): Promise<CliResult> {
    return this.deps.transport.invoke('extension_action', {
      harness,
      action,
      dryRun: false,
      project: null,
    });
  }
}

function namesOf(names: string[]): string {
  const shown = names.slice(0, NAMES_SHOWN).join(', ');

  return names.length > NAMES_SHOWN ? `${shown}, …` : shown;
}

export const extensionUpdater = new ExtensionUpdater({
  transport: ipc,
  installed: () => harness.installed,
  enabled: () => prefs.value?.autoUpdateExtensions ?? false,
  busy: () => actionRunner.phase !== 'idle',
  upgraded: async (id) => {
    await extensionsView.refresh({ quiet: true });
    await reloadChatsAfterPluginChange(id);
  },
  notifications: toasts,
});
