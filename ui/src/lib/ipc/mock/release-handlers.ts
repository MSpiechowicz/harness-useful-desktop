import type { EcosystemId } from '../bindings/EcosystemId';
import type { EcosystemRelease } from '../bindings/EcosystemRelease';
import type { MockState } from './state';
import type { Handlers } from './types';

/**
 * The latest GitHub release of each companion in the mock: the skills are ahead of the 0.2.36 in
 * `fixtures/ecosystem/status.json`, the dashboard and the memory server match their installed 1.22.1 and 1.0.0.
 */
export const MOCK_LATEST_RELEASES: Record<EcosystemId, string> = {
  dashboard: '1.22.1',
  memory: '1.0.0',
  skills: '0.2.40',
  sidebar: '1.0.74',
};

/** Numeric dot-separated parts of a version (a leading `v` ignored), or null when any part is not plain digits. */
function parts(version: string): number[] | null {
  const split = version.replace(/^v/, '').split('.');

  return split.every((part) => /^\d+$/.test(part)) ? split.map(Number) : null;
}

/** Like the backend: only a higher, plainly numeric version is newer (a missing part counts as zero). */
function isNewer(candidate: string, current: string): boolean {
  const [a, b] = [parts(candidate), parts(current)];
  if (a === null || b === null) {
    return false;
  }

  for (let index = 0; index < Math.max(a.length, b.length); index++) {
    const [x, y] = [a[index] ?? 0, b[index] ?? 0];

    if (x !== y) {
      return x > y;
    }
  }

  return false;
}

/** `ecosystem_releases` like the real backend: nothing while `checkCompanionUpdates` is off, one release per installed companion. */
export function releaseHandlers(state: MockState, latest: () => Record<EcosystemId, string> = () => MOCK_LATEST_RELEASES): Pick<Handlers, 'ecosystem_releases'> {
  return {
    ecosystem_releases: ({ installed }) => {
      if (!state.prefs.checkCompanionUpdates) {
        return [];
      }

      const releases: EcosystemRelease[] = [];

      for (const { id, version } of installed) {
        const current = version.trim();

        if (current !== '' && !releases.some((release) => release.id === id)) {
          const latestVersion = latest()[id];
          releases.push({ id, latestVersion, updateAvailable: isNewer(latestVersion, current) });
        }
      }

      return releases;
    },
  };
}
