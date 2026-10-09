import { describe, expect, it } from 'vitest';

import type { PluginInfo } from '$lib/ipc/bindings/PluginInfo';

import { pluginActions } from './item-actions';

function plugin(overrides: Partial<PluginInfo> = {}): PluginInfo {
  return {
    id: 'demo-plugin@demo',
    name: 'demo-plugin',
    version: '1.2.0',
    description: null,
    enabled: true,
    scope: 'user',
    marketplace: 'demo',
    features: [],
    path: null,
    latestVersion: null,
    updateAvailable: false,
    ...overrides,
  };
}

describe('pluginActions', () => {
  it('offers no upgrade while the catalog has nothing newer', () => {
    const labels = pluginActions('omp', plugin(), null).map((action) => action.label);

    expect(labels).toEqual(['Disable', 'Uninstall']);
  });

  it('offers the upgrade, naming the version, once the catalog is ahead', () => {
    const labels = pluginActions('omp', plugin({ latestVersion: '1.3.0', updateAvailable: true }), null).map((action) => action.label);

    expect(labels).toEqual(['Disable', 'Upgrade to v1.3.0', 'Uninstall']);
  });
});
