import { t } from '$lib/i18n/i18n.svelte';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { HarnessInstalled } from '$lib/ipc/bindings/HarnessInstalled';
import type { HarnessRelease } from '$lib/ipc/bindings/HarnessRelease';
import { ipc } from '$lib/ipc/client';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import { Flight } from '$lib/utils/flight';

import { harness, type HarnessStore } from './harness.svelte';
import { toasts, type ToastInput, type ToastsStore } from './toasts.svelte';

/** How long the "updated" toast stays: the toasts' usual time. */
const DONE_TOAST_MS = 5_000;

/** What the latest lookup said about one harness, and the version it was asked with. */
interface Lookup {
  release: HarnessRelease;
  asked: string;
}

/**
 * Newer versions of the harness CLIs the user has installed, and the update that runs in the app.
 * The backend looks them up (and keeps the answer for hours), so asking again costs nothing: the store asks whenever
 * what it knows could be out of date.
 */
export class HarnessUpdatesStore {
  /** The harnesses whose update is running right now. */
  updating = $state<HarnessId[]>([]);

  private found = $state<Lookup[]>([]);
  private readonly flight = new Flight();
  private again = false;
  /** The end of the last update asked for: the next one starts after it. */
  private turns: Promise<void> = Promise.resolve();
  private generation = 0;
  /** The harnesses that got an offer toast in this run: once each, however the toast ended and whatever version came later. */
  private readonly suggested = new Set<HarnessId>();
  /** The offer toasts that may still be shown: one goes away when its update starts or its offer no longer holds. */
  private readonly offerToasts = new Map<HarnessId, number>();

  constructor(
    private readonly transport: Transport,
    private readonly harnesses: Pick<HarnessStore, 'installed' | 'active' | 'get' | 'adoptUpdated'>,
    private readonly notifications: Pick<ToastsStore, 'push' | 'update' | 'has' | 'dismiss'>,
  ) {}

  /** The newer release of an installed harness, or undefined. Once the installed version is not the one asked about (an update made in a terminal), it is gone. */
  offer(id: HarnessId): HarnessRelease | undefined {
    const lookup = this.found.find((each) => each.release.harness === id);

    if (!lookup?.release.updateAvailable || this.harnesses.get(id)?.detection.version !== lookup.asked) {
      return undefined;
    }

    return lookup.release;
  }

  isUpdating(id: HarnessId): boolean {
    return this.updating.includes(id);
  }

  /**
   * Looks up the latest versions of the installed harnesses. A request that comes in while one runs is answered by a
   * second lookup right after it. A failure keeps what is known and says nothing: the next look tries again.
   */
  refresh(): Promise<void> {
    const running = this.flight.running;

    if (running) {
      this.again = true;

      return running;
    }

    return this.flight.start(() => this.lookUp());
  }

  /** Offers the update of the harness in use with a toast, once per run, when there is one. */
  async suggest(id: HarnessId): Promise<void> {
    if (this.suggested.has(id)) {
      return;
    }

    await this.refresh();

    // The user may have moved to another harness while the lookup ran, or may be updating this one already.
    if (this.suggested.has(id) || this.isUpdating(id) || this.harnesses.active?.info.id !== id) {
      return;
    }

    const release = this.offer(id);

    if (!release) {
      return;
    }

    this.suggested.add(id);

    const toast = this.notifications.push({
      title: t('harness.update.offerTitle', { name: this.nameOf(id), version: release.latestVersion }),
      description: t('harness.update.offerBody', { current: this.harnesses.get(id)?.detection.version ?? '' }),
      duration: 0,
      action: { label: t('harness.update.action'), run: () => void this.update(id) },
    });

    this.offerToasts.set(id, toast);
  }

  /**
   * Runs the harness's own updater and shows how it went in one toast. One run per harness at a time. The backend runs
   * one update at a time across all harnesses, so a click during another update waits its turn, and counts as updating
   * (with its progress toast) from the click.
   */
  update(id: HarnessId): Promise<void> {
    if (this.isUpdating(id)) {
      return Promise.resolve();
    }

    const name = this.nameOf(id);
    const release = this.offer(id);

    this.closeOfferToast(id);
    this.updating = [...this.updating, id];

    const toast = this.notifications.push({
      title: t('harness.update.progressTitle', { name }),
      description: t('harness.update.progressBody'),
      duration: 0,
    });

    const run = this.turns.then(() => this.run(id, toast, name, release));

    // The next update waits for this one to end, however it ends.
    this.turns = run.catch(() => {});

    return run;
  }

  private async run(id: HarnessId, toast: number, name: string, release: HarnessRelease | undefined): Promise<void> {
    try {
      const result = await this.transport.invoke('harness_update', { harness: id });

      this.harnesses.adoptUpdated(result.summary);

      if (result.updated) {
        this.settle(toast, {
          title: t('harness.update.doneTitle', { name, version: result.summary.detection.version ?? release?.latestVersion ?? '' }),
          description: t('harness.update.doneBody'),
          tone: 'success',
          duration: DONE_TOAST_MS,
        });
      } else {
        this.settleFailure(toast, name, result.message, result.command);
      }
    } catch (error) {
      this.settleFailure(toast, name, IpcFailure.from(error).message, release?.updateCommand);
    } finally {
      this.updating = this.updating.filter((each) => each !== id);
    }
  }

  private async lookUp(): Promise<void> {
    do {
      this.again = false;

      const asked = this.installedVersions();
      const mine = (this.generation += 1);

      try {
        const releases = asked.length > 0 ? await this.transport.invoke('harness_releases', { installed: asked }) : [];

        // A reply that was overtaken by a newer lookup (this one was abandoned as stale) is dropped.
        if (mine === this.generation) {
          this.found = releases.map((release) => ({ release, asked: asked.find((each) => each.harness === release.harness)?.version ?? '' }));
          this.closeOutdatedOfferToasts();
        }
      } catch {
        // What is shown stays, and the next lookup tries again.
      }
    } while (this.again);
  }

  private closeOfferToast(id: HarnessId): void {
    const toast = this.offerToasts.get(id);

    if (toast !== undefined) {
      this.notifications.dismiss(toast);
      this.offerToasts.delete(id);
    }
  }

  /** An offer that no longer holds (updated in a terminal, or the lookup was switched off) takes its toast with it. */
  private closeOutdatedOfferToasts(): void {
    for (const id of this.offerToasts.keys()) {
      if (!this.offer(id)) {
        this.closeOfferToast(id);
      }
    }
  }

  private installedVersions(): HarnessInstalled[] {
    return this.harnesses.installed.flatMap((summary) => (summary.detection.version ? [{ harness: summary.info.id, version: summary.detection.version }] : []));
  }

  private nameOf(id: HarnessId): string {
    return this.harnesses.get(id)?.info.name ?? id;
  }

  private settleFailure(toast: number, name: string, message: string | null | undefined, command: string | undefined): void {
    this.settle(toast, {
      title: t('harness.update.failedTitle', { name }),
      description: message || t('harness.update.failedBody'),
      tone: 'danger',
      duration: 0,
      action: command ? { label: t('harness.update.copyCommand'), run: () => void this.copy(command) } : undefined,
    });
  }

  /** Shows the outcome in the progress toast, or in a new one when the user already closed it. */
  private settle(toast: number, outcome: ToastInput): void {
    if (this.notifications.has(toast)) {
      this.notifications.update(toast, outcome);
    } else {
      this.notifications.push(outcome);
    }
  }

  private async copy(command: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(command);
    } catch {
      this.notifications.push({ title: t('components.copy.failed'), description: t('components.copy.unavailable'), tone: 'warning' });

      return;
    }

    this.notifications.push({ title: t('components.copy.copied'), description: t('harness.update.copied'), tone: 'success' });
  }
}

export const harnessUpdates = new HarnessUpdatesStore(ipc, harness, toasts);
