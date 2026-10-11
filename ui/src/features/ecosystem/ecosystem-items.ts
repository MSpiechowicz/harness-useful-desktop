import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';


/** One companion's state in words, for badges and the illustration's text alternative. */
export function itemState(item: EcosystemItem): 'installed' | 'missing' | 'unsupported' {
  if (!item.supportedOnThisOs) {
    return 'unsupported';
  }

  return item.installed ? 'installed' : 'missing';
}

/** "2 of 3 installed" for the header and the hero. */
export function installedSummary(items: EcosystemItem[]): string {
  const supported = items.filter((item) => item.supportedOnThisOs);
  const installed = supported.filter((item) => item.installed).length;

  return `${installed} of ${supported.length} installed`;
}

export const SHORT_NAMES: Record<EcosystemId, string> = { dashboard: 'Dashboard', skills: 'Skills', sidebar: 'Sidebar' };
