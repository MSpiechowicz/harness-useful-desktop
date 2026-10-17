<script lang="ts">
  import Globe from '@lucide/svelte/icons/globe';

  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { MemoryScopeCounts } from '$lib/ipc/bindings/MemoryScopeCounts';
  import type { ScriptScope } from '$lib/ipc/bindings/ScriptScope';

  interface Props {
    /** The chosen scope; null for all. */
    value: ScriptScope | null;
    /** How many there are of each scope under the other filters; null while the page was not read. */
    counts: MemoryScopeCounts | null;
    onchange: (scope: ScriptScope | null) => void;
  }

  let { value, counts, onchange }: Props = $props();

  type Choice = ScriptScope | 'all';

  // The same menu, first in the row, on both tabs: where an entry applies is the one choice notes and scripts share.
  const options = $derived<{ value: Choice; label: string; count: number | undefined }[]>([
    { value: 'all', label: t('memory.filter.all'), count: counts ? counts.repo + counts.global : undefined },
    { value: 'repo', label: t('memory.scripts.chips.repo'), count: counts?.repo },
    { value: 'global', label: t('memory.scripts.row.global'), count: counts?.global },
  ]);
</script>

<FilterMenu
  label={t('memory.filter.scope')}
  prefix={t('memory.filter.scope')}
  icon={Globe}
  {options}
  value={value ?? 'all'}
  onchange={(next) => onchange(next === 'all' ? null : next)}
  onclear={() => onchange(null)}
  clearLabel={t('memory.filter.clearScope')}
  align="start"
/>
