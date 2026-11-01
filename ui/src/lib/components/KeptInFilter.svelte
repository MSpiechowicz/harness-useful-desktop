<script lang="ts" generics="V extends string">
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** The places to narrow to. "All" comes first by itself, with `total` after it when known. A `count` shows after a place. */
    places: { value: V; label: string; count?: number }[];
    total?: number;
    /** The place the list is narrowed to, null for every place. */
    place: V | null;
    /** Called with the place chosen, or null for every place, so the view can go back to its first page. */
    onchange: (place: V | null) => void;
  }

  let { places, total, place, onchange }: Props = $props();

  const options = $derived([{ value: 'all' as V | 'all', label: t('memory.filter.all'), count: total }, ...places]);
</script>

<!-- The same "Kept in" filter wherever a list holds entries of this computer and of the remote database. -->
<FilterMenu
  label={t('memory.filter.place')}
  prefix={t('memory.filter.place')}
  {options}
  value={place ?? 'all'}
  onchange={(next) => onchange(next === 'all' ? null : (next as V))}
  onclear={() => onchange(null)}
  clearLabel={t('memory.filter.clearPlace')}
  align="start"
/>
