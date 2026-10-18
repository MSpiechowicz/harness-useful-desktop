import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
import type { EcosystemRelease } from '$lib/ipc/bindings/EcosystemRelease';
import type { EcosystemStatus } from '$lib/ipc/bindings/EcosystemStatus';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { Flight } from '$lib/utils/flight';

import type { SlotStatus } from './dashboard.svelte';
import { COMPANION_CHECK_MS } from './live';

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
  /** Whether the latest release lookup of the current reading is still running. */
  releasesPending = $state(false);

  /** The detection result as read, before the releases were folded in: what a quiet read compares its answer with. */
  private detected: EcosystemStatus | null = null;
  private loadedAt = 0;
  private token = 0;
  private lookup: Promise<void> = Promise.resolve();
  private readonly flight = new Flight();

  constructor(
    private readonly transport: Transport,
    private readonly now: () => number = Date.now,
  ) {}

  /** Forgets the last reading, as at startup (and between tests). */
  reset(): void {
    this.token += 1;
    this.status = null;
    this.detected = null;
    this.error = null;
    this.state = 'idle';
    this.loadedAt = 0;
    this.releasesPending = false;
    this.lookup = Promise.resolve();
    this.flight.clear();
  }

  /** Like `load`, but ends only once the release lookup of that reading did too, so `updateAvailable` is as current as it gets. */
  async loadWithReleases(force = false): Promise<void> {
    await this.load(force);
    await this.lookup;
  }

  /** Whether the ecosystem reading says the installed `id` has a newer release. False when it is not installed or nothing is known. */
  hasUpdate(id: EcosystemId): boolean {
    const item = this.status?.items.find((candidate) => candidate.id === id);

    return item?.installed === true && item.updateAvailable;
  }

  /**
   * Reads the detection result unless a fresh one is held; `force` always reads. A `quiet` read shows no loading state,
   * so a failure on screen stays until it succeeds, and a failure while there is a reading to show keeps that reading.
   * Only a forced, not quiet read (the user asked) interrupts a read that is running: any other waits for it.
   */
  load(force = false, { quiet = false } = {}): Promise<void> {
    if (!force && this.status && this.now() - this.loadedAt < FRESH_MS) {
      return Promise.resolve();
    }

    if (quiet || !force) {
      return this.flight.startUnlessRunning(() => this.read(quiet));
    }

    return this.flight.start(() => this.read(quiet));
  }

  private async read(quiet: boolean): Promise<void> {
    this.token += 1;
    const mine = this.token;
    if (!quiet) {
      this.state = 'loading';
      this.releasesPending = false;
    }

    try {
      const status = await this.transport.invoke('ecosystem_status', undefined);

      if (mine === this.token) {
        const detected = this.apply(status, quiet);

        this.error = null;
        this.state = 'ready';
        this.loadedAt = this.now();
        this.lookup = this.lookUpReleases(detected, mine, quiet);
      }
    } catch (error) {
      if (mine !== this.token) {
        return;
      }

      // The lookup of a reading this one replaced will not report back.
      this.releasesPending = false;

      if (quiet && this.status !== null) {
        // The reading on screen stays, and a read of the user's that was given up on no longer holds the loading state.
        if (this.state === 'loading') {
          this.state = 'ready';
        }

        return;
      }

      this.status = null;
      this.detected = null;
      this.error = IpcFailure.from(error).message;
      this.state = 'error';
    }
  }

  /** A quiet read that found what it found before keeps the reading on screen, release information included, so the update badge never blinks. */
  private apply(status: EcosystemStatus, quiet: boolean): EcosystemStatus {
    if (quiet && this.status !== null && this.detected !== null && JSON.stringify(status) === JSON.stringify(this.detected)) {
      return this.detected;
    }

    this.detected = status;
    this.status = status;

    return status;
  }

  /**
   * Asks the backend for the companions' latest GitHub releases once the local reading is shown. The backend skips
   * the lookup when the setting is off and caches it; any failure only means no update badge, so it is never shown.
   */
  private async lookUpReleases(status: EcosystemStatus, mine: number, quiet: boolean): Promise<void> {
    const installed = status.items.filter((item) => item.installed && item.version).map((item) => ({ id: item.id, version: item.version as string }));

    if (installed.length === 0) {
      this.releasesPending = false;

      return;
    }

    // A background look is silent: the lookup indicator is for the one the user is waiting on.
    if (!quiet) {
      this.releasesPending = true;
    }

    try {
      const releases = await this.transport.invoke('ecosystem_releases', { installed });

      if (mine === this.token && this.detected === status) {
        const merged = withReleases(status, releases);

        if (JSON.stringify(merged) !== JSON.stringify(this.status)) {
          this.status = merged;
        }
      }
    } catch {
      // No badge is the whole fallback.
    } finally {
      if (mine === this.token) {
        this.releasesPending = false;
      }
    }
  }
}

export const ecosystem = new EcosystemStore(ipc);

/**
 * The job that keeps update information current without any view open: reads the companions once now, then every half
 * hour while the window is visible. Returns the function that stops it.
 */
export function watchCompanions(store: EcosystemStore = ecosystem, ms: number = COMPANION_CHECK_MS): () => void {
  void store.load();

  const timer = setInterval(() => {
    if (!document.hidden) {
      void store.load(true, { quiet: true });
    }
  }, ms);

  return () => clearInterval(timer);
}
