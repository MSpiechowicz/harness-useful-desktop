<script lang="ts" module>
  import type { MemoryPlaceCounts } from '$lib/ipc/bindings/MemoryPlaceCounts';
  import type { ScriptPlace } from '$lib/ipc/bindings/ScriptPlace';

  /** The place the table is narrowed to. An older Harness Memory cannot filter by place (`places` is null): it answers every place, so the choice is not applied. */
  export function placeApplied(places: MemoryPlaceCounts | null | undefined, place: ScriptPlace | null): boolean {
    return places != null && place !== null;
  }
</script>

<script lang="ts">
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import { PLACE_LABELS } from './script-format';

  type PlaceChoice = ScriptPlace | 'all';

  interface Props {
    /** How many are kept in each place under the other filters; null from a Harness Memory that cannot filter by place. */
    places: MemoryPlaceCounts | null;
    place: ScriptPlace | null;
    /** Called with the place chosen, or null for both, so the view can go back to its first page. */
    onchange: (place: ScriptPlace | null) => void;
  }

  let { places, place, onchange }: Props = $props();


  const options = $derived([
    { value: 'all' as PlaceChoice, label: t('memory.filter.all'), count: places ? places.local + places.team : undefined },
    { value: 'local' as PlaceChoice, label: t(PLACE_LABELS.local), count: places?.local },
    { value: 'team' as PlaceChoice, label: t(PLACE_LABELS.team), count: places?.team },
  ]);
</script>

{#if places !== null}
  <FilterMenu
    label={t('memory.filter.place')}
    prefix={t('memory.filter.place')}
    {options}
    value={place ?? 'all'}
    onchange={(next) => onchange(next === 'all' ? null : next)}
    onclear={() => onchange(null)}
    clearLabel={t('memory.filter.clearPlace')}
    align="start"
  />
{/if}
