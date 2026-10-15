<script lang="ts">
  import type { Snippet } from 'svelte';

  import { SETUP_COLUMNS } from './layout';
  import SetupChip from './SetupChip.svelte';
  import type { Chip } from './setup-model';

  interface Props {
    /** A product mark, one control wide. */
    lead: Snippet;
    /** What the line is called. */
    name: Snippet;
    /** The state of the line's item; absent where the line only names a card of rows. */
    chip?: Chip;
  }

  let { lead, name, chip }: Props = $props();
</script>

<!-- The header line of a card of the step: mark, name and state chip, each centred on the line. -->
<div class={SETUP_COLUMNS}>
  <span class="grid place-items-center">{@render lead()}</span>

  <div class="min-w-0">{@render name()}</div>

  {#if chip}
    <SetupChip {chip} />
  {:else}
    <span></span>
  {/if}

</div>
