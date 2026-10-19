import type { HarnessId } from '$lib/ipc/bindings/HarnessId';
import { harness } from '$lib/stores/harness.svelte';

/** The harness's full name, or its id in capitals while the harness list has not arrived. */
export function nameOf(id: HarnessId): string {
  return harness.get(id)?.info.name ?? id.toUpperCase();
}

export function badgeOf(id: HarnessId): { id: HarnessId; shortName: string } {
  return harness.get(id)?.info ?? { id, shortName: id.toUpperCase() };
}
