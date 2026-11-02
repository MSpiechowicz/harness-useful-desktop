import { FAMILY } from '$lib/brand/family';
import { t, tn } from '$lib/i18n/i18n.svelte';
import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';

import { itemState, SHORT_NAMES } from '../ecosystem-items';

export type ModuleState = 'installed' | 'update' | 'missing';

/** What the diagram knows of one Harness family member. */
export interface KernelMember {
  id: EcosystemId;
  name: string;
  short: string;
  state: ModuleState;
  /** The member's own colour: the midpoint of the two stops of its mark's gradient, darkened in the light theme by `--kernel-mix`. */
  colour: string;
}

/** The members that can run on this machine, in the order the ecosystem lists them. A tool this OS cannot run gets no socket. */
export function kernelMembers(items: EcosystemItem[]): KernelMember[] {
  return items
    .filter((item) => item.supportedOnThisOs)
    .map((item) => ({
      id: item.id,
      name: item.name,
      short: SHORT_NAMES[item.id],
      state: stateOf(item),
      colour: `color-mix(in srgb, color-mix(in srgb, ${FAMILY[item.id][0]}, ${FAMILY[item.id][1]}) var(--kernel-mix), black)`,
    }));
}

function stateOf(item: EcosystemItem): ModuleState {
  if (itemState(item) !== 'installed') {
    return 'missing';
  }

  return item.updateAvailable ? 'update' : 'installed';
}

/** "5 of 6 modules loaded · 1 update", the plain reading of the whole diagram. */
export function kernelSummary(members: KernelMember[]): string {
  if (members.length === 0) {
    return '';
  }

  const loaded = members.filter((member) => member.state !== 'missing').length;
  const updates = members.filter((member) => member.state === 'update').length;
  const parts = [tn('ecosystem.kernel.loaded', members.length, { loaded }), updates > 0 ? tn('ecosystem.kernel.updates', updates) : null];

  return parts.filter(Boolean).join(' · ');
}

/** The state in words, the same ones the cards use. */
export function stateLabel(state: ModuleState): string {
  switch (state) {
    case 'update':
      return t('ecosystem.card.updateAvailable');

    case 'installed':
      return t('ecosystem.card.installed');

    default:
      return t('ecosystem.card.notInstalled');
  }
}

/** The status line while a module is hovered or focused: "Harness Knowledge · Update available". */
export function focusLine(name: string, state: ModuleState): string {
  return t('ecosystem.kernel.focus', { name, state: stateLabel(state) });
}

/** The text alternative of one module, also the accessible name of its button. */
export function moduleLabel(member: Pick<KernelMember, 'name' | 'state'>): string {
  switch (member.state) {
    case 'update':
      return t('ecosystem.kernel.updating', { name: member.name });

    case 'installed':
      return t('ecosystem.kernel.installed', { name: member.name });

    default:
      return t('ecosystem.kernel.missing', { name: member.name });
  }
}
