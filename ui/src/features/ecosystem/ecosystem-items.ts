import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
import { t, tn } from '$lib/i18n/i18n.svelte';
import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';

import { COMPANIONS, type Companion } from '../onboarding/setup-model';

/** One companion's state in words, for badges and the illustration's text alternative. */
export function itemState(item: EcosystemItem): 'installed' | 'missing' | 'unsupported' {
  if (!item.supportedOnThisOs) {
    return 'unsupported';
  }

  return item.installed ? 'installed' : 'missing';
}

/** "2 of 4 installed" for the header and the hero. */
export function installedSummary(items: EcosystemItem[]): string {
  const supported = items.filter((item) => item.supportedOnThisOs);
  const installed = supported.filter((item) => item.installed).length;

  return t('ecosystem.summary.installed', { installed, total: supported.length });
}

/** "1 update available", or nothing when every installed companion is current. */
export function updatesSummary(items: EcosystemItem[]): string | null {
  const count = items.filter((item) => itemState(item) === 'installed' && item.updateAvailable).length;

  return count === 0 ? null : tn('ecosystem.count.update', count);
}

export const SHORT_NAMES: Record<EcosystemId, string> = { dashboard: 'Dashboard', memory: 'Memory', knowledge: 'Knowledge', code: 'Code', skills: 'Skills', sidebar: 'Sidebar' };

/** The id of a card's title: the diagram of the family finds the card of a module by it. */
export function cardTitleId(id: EcosystemId): string {
  return `ecosystem-${id}`;
}

/** The member's own installer the app can run. The others (a plugin of a harness) only have steps to copy. */
export function rowInstaller(id: EcosystemId): Companion | null {
  return (COMPANIONS as readonly EcosystemId[]).includes(id) ? (id as Companion) : null;
}

/** The row that starts open: the first member with an update waiting. */
export function firstUpdating(items: EcosystemItem[]): EcosystemId | null {
  return items.find((item) => itemState(item) === 'installed' && item.updateAvailable)?.id ?? null;
}
