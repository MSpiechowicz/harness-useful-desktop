<script lang="ts">
  import SetupChip from './SetupChip.svelte';
  import type { Chip } from './setup-model';

  interface Props {
    /** The product the cell is about, for assistive technology: the column's title. */
    label: string;
    chip: Chip;
    /** Where the item lives, shown as the cell's tooltip. */
    hint?: string;
  }

  let { label, chip, hint }: Props = $props();
</script>

<!-- One state of one harness, like a table cell: the chip starts under its column's title. -->
<span role="group" aria-label={label} title={hint} class="flex items-center">
  <SetupChip {chip} />
</span>
