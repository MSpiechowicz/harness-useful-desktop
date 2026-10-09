import { harness } from '$lib/stores/harness.svelte';
import { project } from '$lib/stores/project.svelte';

/** "OMP · demo": which harness and project a view is showing, for header context lines. */
export function scopeLabel(...extra: (string | null | undefined)[]): string {
  const parts = [harness.active?.info.shortName ?? 'No harness', project.active?.name ?? 'no project', ...extra];

  return parts.filter((part): part is string => Boolean(part)).join(' · ');
}

/** "3 sessions" / "1 session". */
export function count(amount: number, noun: string): string {
  return `${amount} ${noun}${amount === 1 ? '' : 's'}`;
}
