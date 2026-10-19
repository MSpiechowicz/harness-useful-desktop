<script lang="ts">
  import type { Snippet } from 'svelte';

  import DocNewButton from '$lib/components/docs/DocNewButton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';

  import { viewMeta } from '../shell/views';
  import DeleteAllMenu from './DeleteAllMenu.svelte';

  interface Props {
    /** The view this is the header of: it names it, and says what "Delete all" deletes. */
    view: 'notes' | 'scripts';
    /** The id of the place in the header that the list on screen puts its search and its filters into. */
    slot: string;
    /** What the add button says. */
    addLabel: string;
    /** There is a Harness Memory to save to: without one the add button is not offered. */
    available: boolean;
    onadd: () => void;
    /** More items of the "⋯" menu, above "Delete all". They get the menu's `close`. */
    extra?: Snippet<[{ close: () => void }]>;
  }

  let { view, slot, addLabel, available, onadd, extra }: Props = $props();

  const meta = $derived(viewMeta(view));
</script>

<ViewHeader title={meta.label} icon={meta.icon}>
  {#snippet actions()}
    <div id={slot} class="contents"></div>

    <!-- After the search and the filters. -->
    {#if available}
      <DocNewButton label={addLabel} onclick={onadd} />
    {/if}
  {/snippet}

  {#snippet fixed()}
    <DeleteAllMenu section={view} {extra} />
  {/snippet}
</ViewHeader>
