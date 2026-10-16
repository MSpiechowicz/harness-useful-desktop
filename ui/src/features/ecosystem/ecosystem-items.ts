import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
import { t, tn } from '$lib/i18n/i18n.svelte';
import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';

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

export const SHORT_NAMES: Record<EcosystemId, string> = { dashboard: 'Dashboard', memory: 'Memory', skills: 'Skills', sidebar: 'Sidebar' };
