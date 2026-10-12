import { i18n, tMaybe } from '$lib/i18n/i18n.svelte';
import type { EcosystemId } from '$lib/ipc/bindings/EcosystemId';
import type { EcosystemItem } from '$lib/ipc/bindings/EcosystemItem';

/**
 * Our own product copy comes from the backend's catalog in English. Another language shows its translation of the
 * same entry (keyed by product and position); English, and any entry the translations do not know yet, shows the
 * backend's text as received.
 */
function copy(id: EcosystemId, part: string, received: string): string {
  if (i18n.lang === 'en') {
    return received;
  }

  return tMaybe(`ecosystem.product.${id}.${part}`) ?? received;
}

export function productTagline(item: EcosystemItem): string {
  return copy(item.id, 'tagline', item.tagline);
}

export function productDescription(item: EcosystemItem): string {
  return copy(item.id, 'description', item.description);
}

export function productNote(item: EcosystemItem, index: number): string {
  return copy(item.id, `note.${index}`, item.notes[index]);
}

/** The title of install step `index`, as the backend lists them. */
export function productStepTitle(id: EcosystemId, index: number, received: string): string {
  return copy(id, `step.${index}.title`, received);
}
