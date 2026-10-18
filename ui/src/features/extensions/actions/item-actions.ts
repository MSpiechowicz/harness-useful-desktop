import ArrowUpFromLine from '@lucide/svelte/icons/arrow-up-from-line';
import Power from '@lucide/svelte/icons/power';
import PowerOff from '@lucide/svelte/icons/power-off';
import Trash2 from '@lucide/svelte/icons/trash-2';

import { t } from '$lib/i18n/i18n.svelte';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { InstallScope } from '$lib/ipc/bindings/InstallScope';
import type { MarketplaceInfo } from '$lib/ipc/bindings/MarketplaceInfo';
import type { PackageInfo } from '$lib/ipc/bindings/PackageInfo';
import type { PluginInfo } from '$lib/ipc/bindings/PluginInfo';

import { actionRequest, type BarAction } from './action-model';

function scopeOf(scope: string): InstallScope {
  return scope === 'project' ? 'project' : 'user';
}

/** The request that updates an installed plugin, in the scope it was installed in. */
export function pluginUpdateRequest(harness: HarnessId, plugin: PluginInfo, project: string | null) {
  return actionRequest(harness, { type: 'pluginUpgrade', name: plugin.id }, scopeOf(plugin.scope), project);
}

/**
 * Enable or disable, upgrade and uninstall of an installed plugin. Upgrade is offered only when the marketplace
 * catalog has a newer version: the CLI would otherwise reinstall the same version and call it a success.
 */
export function pluginActions(harness: HarnessId, plugin: PluginInfo, project: string | null): BarAction[] {
  const scope = scopeOf(plugin.scope);
  const request = (action: Parameters<typeof actionRequest>[1]) => actionRequest(harness, action, scope, project);
  const actions: BarAction[] = [
    plugin.enabled
      ? {
          label: t('extensions.verb.disable'),
          title: t('extensions.action.disable.title'),
          description: t('extensions.action.disable.description'),
          icon: PowerOff,
          request: request({ type: 'pluginDisable', name: plugin.id }),
        }
      : {
          label: t('extensions.verb.enable'),
          title: t('extensions.action.enable.title'),
          description: t('extensions.action.enable.description'),
          icon: Power,
          request: request({ type: 'pluginEnable', name: plugin.id }),
        },
  ];

  // Always listed, so the row is where the user expects it: blue when there is a newer version, greyed out when there is not.
  actions.push({
    label: t('extensions.verb.update'),
    title: plugin.updateAvailable ? t('extensions.action.upgrade.title') : t('extensions.action.upgrade.none.title'),
    description: !plugin.updateAvailable
      ? t('extensions.action.upgrade.none.description')
      : plugin.latestVersion
        ? t('extensions.action.upgrade.version', { version: plugin.latestVersion })
        : t('extensions.action.upgrade.newer'),
    icon: ArrowUpFromLine,
    state: plugin.updateAvailable ? 'ready' : 'idle',
    request: pluginUpdateRequest(harness, plugin, project),
  });

  actions.push({
    label: t('extensions.verb.uninstall'),
    title: t('extensions.action.uninstall.title'),
    description: t('extensions.action.uninstall.description'),
    icon: Trash2,
    danger: true,
    request: request({ type: 'pluginUninstall', name: plugin.id }),
  });

  return actions;
}

/** Remove a marketplace. Its catalog needs no action: the app re-reads every catalog by itself. */
export function marketplaceActions(harness: HarnessId, marketplace: MarketplaceInfo): BarAction[] {
  return [
    {
      label: t('extensions.verb.remove'),
      title: t('extensions.verb.removeMarketplace'),
      description: t('extensions.action.removeMarketplace.description'),
      icon: Trash2,
      danger: true,
      request: actionRequest(harness, { type: 'marketplaceRemove', name: marketplace.name }, 'user', null),
    },
  ];
}

/** Update or remove an installed package; nothing for a package whose source is hidden. */
export function packageActions(harness: HarnessId, pkg: PackageInfo, project: string | null): BarAction[] {
  if (pkg.source.kind !== 'text') {
    return [];
  }

  const source = pkg.source.value;
  const scope = scopeOf(pkg.scope);

  return [
    {
      label: t('extensions.verb.update'),
      title: t('extensions.action.updatePackage.title'),
      description: t('extensions.action.updatePackage.description'),
      icon: ArrowUpFromLine,
      request: actionRequest(harness, { type: 'packageUpdate', source }, scope, project),
    },
    {
      label: t('extensions.verb.uninstall'),
      title: t('extensions.action.removePackage.title'),
      description: t('extensions.action.removePackage.description'),
      icon: Trash2,
      danger: true,
      request: actionRequest(harness, { type: 'packageRemove', source }, scope, project),
    },
  ];
}

/** The install request for one catalog entry, in the chosen scope. */
export function entryInstall(harness: HarnessId, marketplace: string, entry: string, scope: InstallScope, project: string | null) {
  return actionRequest(harness, { type: 'pluginInstall', target: `${entry}@${marketplace}`, scope }, scope, project);
}
