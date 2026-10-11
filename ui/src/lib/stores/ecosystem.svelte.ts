import type { EcosystemRelease } from '$lib/ipc/bindings/EcosystemRelease';
import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';

import type { SlotStatus } from './dashboard.svelte';

/** A status read this recent is shown as is when the view is opened again. */
const FRESH_MS = 30_000;

/** Numeric dot parts in order ("v1.10.0" → [1, 10, 0]); null when any part is not a number. */
function versionParts(version: string): number[] | null {
  const parts = version.replace(/^v/, '').split('.');

  return parts.every((part) => /^\d+$/.test(part)) ? parts.map(Number) : null;
}

/** Whether `a` is a higher version than `b`; anything that is not plain numbers never counts as higher. */
export function isHigherVersion(a: string, b: string): boolean {
  const left = versionParts(a);
  const right = versionParts(b);

  if (!left || !right) {
    return false;
  }

  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const difference = (left[index] ?? 0) - (right[index] ?? 0);

    if (difference !== 0) {
      return difference > 0;
    }
  }

  return false;
}

/**
 * The detection result with the GitHub releases folded in: a release newer than the installed version marks its
 * companion as outdated and names that version. A companion GitHub says nothing about keeps what OMP's catalog said.
 */
export function withReleases(status: EcosystemStatus, releases: EcosystemRelease[]): EcosystemStatus {
  const newer = new Map(releases.filter((release) => release.updateAvailable).map((release) => [release.id, release.latestVersion]));

  if (newer.size === 0) {
    return status;
  }

  return {
    ...status,
    items: status.items.map((item) => {
      const latest = newer.get(item.id);

      if (latest === undefined) {
        return item;
      }

      // OMP's catalog may already know a higher version than GitHub's latest release; the card names the higher one.
      const highest = item.latestVersion && isHigherVersion(item.latestVersion, latest) ? item.latestVersion : latest;

      return { ...item, updateAvailable: true, latestVersion: highest };
    }),
  };
}

/** Which companion tools of the Harness Useful ecosystem are installed on this machine. */
export class EcosystemStore {
  status = $state.raw<EcosystemStatus | null>(null);
  state = $state<SlotStatus>('idle');
  error = $state<string | null>(null);

  private loadedAt = 0;
  private token = 0;

  constructor(
    private readonly transport: Transport,
    private readonly now: () => number = Date.now,
  ) {}

  /** Forgets the last reading, as at startup (and between tests). */
  reset(): void {
    this.token += 1;
    this.status = null;
    this.error = null;
    this.state = 'idle';
    this.loadedAt = 0;
  }

  /** Reads the detection result unless a fresh one is held; `force` always reads. */
  async load(force = false): Promise<void> {
    if (!force && this.status && this.now() - this.loadedAt < FRESH_MS) {
      return;
    }

    this.token += 1;
    const mine = this.token;
    this.state = 'loading';

    try {
      const status = await this.transport.invoke('ecosystem_status', undefined);

      if (mine === this.token) {
        this.status = status;
        this.error = null;
        this.state = 'ready';
        this.loadedAt = this.now();
        void this.lookUpReleases(status, mine);
      }
    } catch (error) {
      if (mine === this.token) {
        this.status = null;
        this.error = IpcFailure.from(error).message;
        this.state = 'error';
      }
    }
  }

  /**
   * Asks the backend for the companions' latest GitHub releases once the local reading is shown. The backend skips
   * the lookup when the setting is off and caches it; any failure only means no update badge, so it is never shown.
   */
  private async lookUpReleases(status: EcosystemStatus, mine: number): Promise<void> {
    const installed = status.items.filter((item) => item.installed && item.version).map((item) => ({ id: item.id, version: item.version as string }));

    if (installed.length === 0) {
      return;
    }

    try {
      const releases = await this.transport.invoke('ecosystem_releases', { installed });

      if (mine === this.token && this.status === status) {
        this.status = withReleases(status, releases);
      }
    } catch {
      // No badge is the whole fallback.
    }
  }
}

export const ecosystem = new EcosystemStore(ipc);
