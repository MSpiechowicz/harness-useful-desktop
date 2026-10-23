import { t } from '$lib/i18n/i18n.svelte';
import { ipc } from '$lib/ipc/client';
import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import type { SkillsMigrationReport } from '$lib/ipc/bindings/SkillsMigrationReport';
import type { Transport } from '$lib/ipc/transport';
import { ecosystem } from '$lib/stores/ecosystem.svelte';
import { toasts, type ToastsStore } from '$lib/stores/toasts.svelte';

import { HARNESS_NAMES } from '../routines/routine-format';

interface Dependencies {
  transport: Transport;
  /** Called after a harness was migrated, so the ecosystem reading catches up with the renamed install. */
  migrated: () => Promise<void>;
  notifications: Pick<ToastsStore, 'push'>;
}

/**
 * Asks the backend, once per launch, to move Harness Skills installed under their names before 0.4.0 to the new ones.
 * The backend decides everything (automatic updates on, the install found only under the old names, a release that has
 * the new names, once per change) and answers what it did. One quiet notice names the harnesses, and the failures with
 * their reasons. Nothing is shown when there was nothing to do.
 */
export class SkillsNamesMigration {
  private asked = false;

  constructor(private readonly deps: Dependencies) {}

  /** Never throws. A second call in the same launch does nothing. */
  async run(): Promise<void> {
    if (this.asked) {
      return;
    }

    this.asked = true;

    try {
      const report = await this.deps.transport.invoke('skills_migrate_names', undefined);

      if (report.migrated.length === 0 && report.failed.length === 0) {
        return;
      }

      this.announce(report);

      if (report.migrated.length > 0) {
        await this.deps.migrated();
      }
    } catch {
      // A failed call is not worth a notice: the next launch asks again.
    }
  }

  private announce(report: SkillsMigrationReport): void {
    const names = (ids: readonly HarnessId[]) => ids.map((id) => HARNESS_NAMES[id]).join(', ');

    if (report.failed.length === 0) {
      this.deps.notifications.push({ title: t('ecosystem.skillsMigration.done', { harnesses: names(report.migrated) }), tone: 'success' });

      return;
    }

    const reasons = report.failed
      .map((failure) =>
        t('ecosystem.skillsMigration.reason', {
          harness: names([failure.harness]),
          reason: failure.reason || t('ecosystem.skillsMigration.noReason'),
        }),
      )
      .join('\n');

    const title =
      report.migrated.length > 0
        ? t('ecosystem.skillsMigration.done', { harnesses: names(report.migrated) })
        : t('ecosystem.skillsMigration.failed', { harnesses: names(report.failed.map((failure) => failure.harness)) });

    this.deps.notifications.push({ title, description: reasons, tone: 'warning' });
  }
}

export const skillsNamesMigration = new SkillsNamesMigration({
  transport: ipc,
  migrated: () => ecosystem.load(true),
  notifications: toasts,
});
