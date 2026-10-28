import { t } from '$lib/i18n/i18n.svelte';
import type { CliResult } from '$lib/ipc/bindings/CliResult';
import type { ExtensionAction } from '$lib/ipc/bindings/ExtensionAction';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { harness } from '$lib/stores/harness.svelte';
import { CATALOG_REFRESH_MS, EXTENSION_UPDATE_FIRST_DELAY_MS, EXTENSION_UPDATE_INTERVAL_MS, PACKAGE_UPDATE_INTERVAL_MS } from '$lib/stores/live';
import { prefs } from '$lib/stores/prefs.svelte';
import { toasts, type ToastsStore } from '$lib/stores/toasts.svelte';
import { extensionsView } from '$lib/stores/views.svelte';

import { failureLine, installAndRemoveOnly } from './action-model';
import { actionRunner, reloadChatsAfterPluginChange } from './action-runner.svelte';

const NAMES_SHOWN = 3;

interface Dependencies {
  transport: Transport;
  /** The harnesses detected on this machine. */
  installed: () => HarnessSummary[];
  /** Whether installing plugin and package updates is on; read at every look so switching it off takes effect at once. */
  enabled: () => boolean;
  /** Whether the catalogs are refreshed on their own schedule; read at every look. Off means no catalog request unless installing updates is on, which needs them. */
  refreshCatalogs: () => boolean;
  /** Whether the user is in the middle of an extension action of their own; the two must not run the CLI together. */
  busy: () => boolean;
  /** Called after plugins changed, so open chats of the harness pick the change up. */
  upgraded: (harness: HarnessId) => Promise<void>;
  notifications: Pick<ToastsStore, 'push'>;
  now?: () => number;
}

/**
 * Keeps the plugins, marketplace catalogs and packages of every installed harness current without the user doing
 * anything. Goes through the same `extension_action` command as the Extensions buttons.
 *
 * Two settings drive it. Refreshing the catalogs (`refreshCatalogs`) re-reads them and is the only thing that setting
 * does. Installing updates (`enabled`) installs the plugin upgrades the catalogs reveal (announcing the batch in a
 * toast first) and updates packages. Since the catalogs are what reveal upgrades, installing also reads them, even with
 * the refresh setting off. Whichever of the two asks, the catalogs are read at most once per `CATALOG_REFRESH_MS`.
 * With both off nothing is requested at all.
 *
 * A batch is tried once, whatever the outcome, and not again until what is outdated changes, so a plugin that stays
 * outdated does not produce a toast every ten minutes.
 */
export class ExtensionUpdater {
  /** Plugin batches (harness plus ids and versions) already tried, and the package failures already announced. */
  private readonly attempted = new Set<string>();
  private lastPackageRun = new Map<HarnessId, number>();
  private lastCatalogRun = new Map<HarnessId, number>();
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
    if (this.checking || this.deps.busy()) {
      return;
    }

    this.checking = true;

    try {
      for (const summary of this.deps.installed()) {
        if (this.deps.busy()) {
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

    // The plugins are judged by the catalogs, so installing updates needs them too. Either way they are read on one
    // schedule, never more often than `CATALOG_REFRESH_MS`.
    const refresh = this.catalogsWanted() && this.catalogsDue(info.id);

    if (capabilities.marketplaces && refresh) {
      await this.refreshMarketplaces(info.id);
    }

    if (!this.deps.enabled()) {
      return;
    }

    // OpenCode plugins are never reported outdated, so there is nothing to upgrade and no list to read.
    if (capabilities.plugins && !installAndRemoveOnly(info.id)) {
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
   * catalogs say now. A catalog the harness cannot refresh (Codex's built-in ones) is left out: asking could only fail.
   */
  private async refreshMarketplaces(harness: HarnessId): Promise<void> {
    this.lastCatalogRun.set(harness, this.now());

    let names: string[];

    try {
      names = (await this.view(harness)).marketplaces
        .filter((marketplace) => marketplace.refreshable)
        .map((marketplace) => marketplace.name);
    } catch {
      return;
    }

    for (const name of names) {
      // Both settings are read again for every catalog, so switching them off in the middle of a pass stops it at once.
      if (this.deps.busy() || !this.catalogsWanted()) {
        return;
      }

      await this.run(harness, { type: 'marketplaceUpdate', name }).catch(() => null);
    }
  }

  private catalogsWanted(): boolean {
    return this.deps.enabled() || this.deps.refreshCatalogs();
  }

  private catalogsDue(harness: HarnessId): boolean {
    const last = this.lastCatalogRun.get(harness);

    return last === undefined || this.now() - last >= CATALOG_REFRESH_MS;
  }

  private now(): number {
    return (this.deps.now ?? Date.now)();
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
    const now = this.now();

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
  refreshCatalogs: () => prefs.value?.autoRefreshCatalogs ?? false,
  busy: () => actionRunner.phase !== 'idle',
  upgraded: async (id) => {
    await extensionsView.refresh({ quiet: true });
    await reloadChatsAfterPluginChange(id);
  },
  notifications: toasts,
});
