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
  it('keeps the update row but greys it out while there is nothing newer', () => {
    const actions = pluginActions('omp', plugin(), null);

    expect(actions.map((action) => action.label)).toEqual(['Disable', 'Update', 'Uninstall']);
    expect(actions[1].state).toBe('idle');
  });

  it('offers the upgrade, naming the version, once the catalog is ahead', () => {
    const labels = pluginActions('omp', plugin({ latestVersion: '1.3.0', updateAvailable: true }), null).map((action) => action.label);

    expect(labels).toEqual(['Disable', 'Update', 'Uninstall']);
  });

  it('makes the update the blue primary action once a newer version exists', () => {
    const update = pluginActions('omp', plugin({ latestVersion: '1.3.0', updateAvailable: true }), null)[1];

    expect(update.state).toBe('ready');
  });

  it('offers OpenCode plugins nothing but the removal', () => {
    const actions = pluginActions('opencode', plugin({ id: 'opencode-wakatime', marketplace: null }), null);

    expect(actions.map((action) => action.label)).toEqual(['Uninstall']);
    expect(actions[0].request.action).toEqual({ type: 'pluginUninstall', name: 'opencode-wakatime' });
  });
});
