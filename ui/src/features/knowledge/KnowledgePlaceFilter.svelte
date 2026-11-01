<script lang="ts">
  import KeptInFilter from '$lib/components/KeptInFilter.svelte';
  import type { KnowledgePlace } from '$lib/ipc/bindings/KnowledgePlace';

  import { placeLabel } from './knowledge-format';

  interface Props {
    /** The place the documents are narrowed to, null for both. */
    place: KnowledgePlace | null;
    /** Called with the place chosen, or null for both. */
    onchange: (place: KnowledgePlace | null) => void;
  }

  let { place, onchange }: Props = $props();

  const places = $derived((['remote', 'local'] as const).map((each) => ({ value: each, label: placeLabel(each) })));
</script>

<KeptInFilter {places} {place} {onchange} />
