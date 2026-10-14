import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
import { isHigherVersion } from '$lib/stores/ecosystem.svelte';

/**
 * Marks the plugins that are Harness Useful companions with the update their GitHub release announces. OMP's marketplace
 * catalog is only as new as its last refresh, so a plugin can be outdated long before the catalog says so; the release
 * check does not depend on it. A plugin the catalog already knows to be outdated keeps what the catalog said.
 */
export function withCompanionUpdates(view: ExtensionsView, status: EcosystemStatus | null): ExtensionsView {
  const outdated = status?.items.filter((item) => item.updateAvailable && item.latestVersion) ?? [];

  if (outdated.length === 0) {
    return view;
  }

  return {
    ...view,
    plugins: view.plugins.map((plugin) => {
      const latest = outdated.find((item) => item.via.some((line) => line === `OMP plugin ${plugin.id}` || line.startsWith(`OMP plugin ${plugin.id} `)))?.latestVersion;

      if (!latest || (plugin.version && !isHigherVersion(latest, plugin.version))) {
        return plugin;
      }

      // The catalog may know a higher version than the latest release; the plugin names the higher one.
      const highest = plugin.latestVersion && isHigherVersion(plugin.latestVersion, latest) ? plugin.latestVersion : latest;

      return { ...plugin, updateAvailable: true, latestVersion: highest };
    }),
  };
}
