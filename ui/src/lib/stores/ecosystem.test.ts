import { describe, expect, it } from 'vitest';

import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';
import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';

import { isHigherVersion, withReleases } from './ecosystem.svelte';

function item(id: EcosystemItem['id'], version: string | null, latestVersion: string | null = null): EcosystemItem {
  return {
    id,
    name: id,
    tagline: '',
    description: '',
    repoUrl: '',
    docsUrl: '',
    installed: version !== null,
    version,
    latestVersion,
    updateAvailable: latestVersion !== null,
    via: [],
    supportedOnThisOs: true,
    installSteps: [],
    notes: [],
  };
}

const status: EcosystemStatus = { items: [item('dashboard', '1.22.1'), item('skills', '0.2.36'), item('sidebar', null)] };

describe('withReleases', () => {
  it('marks a companion whose GitHub release is newer and names that version', () => {
    const merged = withReleases(status, [{ id: 'dashboard', latestVersion: '1.25.0', updateAvailable: true }]);

    expect(merged.items[0]).toMatchObject({ updateAvailable: true, latestVersion: '1.25.0' });
    expect(merged.items[1]).toBe(status.items[1]);
  });

  it('leaves a companion that is current, or that GitHub said nothing about, as detected', () => {
    const catalog: EcosystemStatus = { items: [item('skills', '0.2.36', '0.2.38')] };
    const merged = withReleases(catalog, [{ id: 'skills', latestVersion: '0.2.36', updateAvailable: false }]);

    expect(merged).toBe(catalog);
    expect(merged.items[0]).toMatchObject({ updateAvailable: true, latestVersion: '0.2.38' });
  });

  it('names the higher version when the catalog is ahead of GitHub', () => {
    const catalog: EcosystemStatus = { items: [item('skills', '0.2.36', '0.2.50')] };
    const merged = withReleases(catalog, [{ id: 'skills', latestVersion: '0.2.40', updateAvailable: true }]);

    expect(merged.items[0]).toMatchObject({ updateAvailable: true, latestVersion: '0.2.50' });
  });

  it('keeps the status as is when there are no releases', () => {
    expect(withReleases(status, [])).toBe(status);
  });
});

describe('isHigherVersion', () => {
  it('compares numeric parts, a missing part counting as zero', () => {
    expect(isHigherVersion('1.10.0', '1.9.9')).toBe(true);
    expect(isHigherVersion('v2', '1.9.9')).toBe(true);
    expect(isHigherVersion('1.0', '1.0.0')).toBe(false);
    expect(isHigherVersion('1.2.0', '1.3.0')).toBe(false);
  });

  it('never ranks a version with letters higher', () => {
    expect(isHigherVersion('1.0.0.beta', '1.0.0')).toBe(false);
    expect(isHigherVersion('1.0.1', 'dev')).toBe(false);
  });
});
