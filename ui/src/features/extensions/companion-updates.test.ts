import { describe, expect, it } from 'vitest';

import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';
import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';

import { withCompanionUpdates } from './companion-updates';

const plugin = (id: string, version: string, extra = {}) => ({ id, name: id.split('@')[0], version, updateAvailable: false, latestVersion: null, ...extra });
const view = (plugins: object[]) => ({ plugins }) as unknown as ExtensionsView;
const status = (via: string[], latestVersion: string | null, updateAvailable = true) =>
  ({ items: [{ id: 'skills', via, latestVersion, updateAvailable }] }) as unknown as EcosystemStatus;

describe('withCompanionUpdates', () => {
  it('marks a companion plugin that GitHub says is outdated, though its catalog has not refreshed', () => {
    const result = withCompanionUpdates(view([plugin('harness-useful-skills@harness-useful-skills', '0.2.41')]), status(['OMP plugin harness-useful-skills@harness-useful-skills v0.2.41'], '0.2.42'));

    expect(result.plugins[0]).toMatchObject({ updateAvailable: true, latestVersion: '0.2.42' });
  });

  it('leaves other plugins, up-to-date ones and a missing release check alone', () => {
    const original = view([plugin('other@market', '1.0.0'), plugin('harness-useful-skills@harness-useful-skills', '0.2.42')]);

    expect(withCompanionUpdates(original, status(['OMP plugin harness-useful-skills@harness-useful-skills v0.2.42'], '0.2.42')).plugins.every((entry) => !entry.updateAvailable)).toBe(true);
    expect(withCompanionUpdates(original, null)).toBe(original);
  });

  it('keeps a higher version the catalog already knew', () => {
    const result = withCompanionUpdates(
      view([plugin('a@a', '1.0.0', { updateAvailable: true, latestVersion: '1.3.0' })]),
      status(['OMP plugin a@a v1.0.0'], '1.2.0'),
    );

    expect(result.plugins[0].latestVersion).toBe('1.3.0');
  });
});
