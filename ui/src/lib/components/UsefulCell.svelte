<script lang="ts">
  import UsageMeter from './UsageMeter.svelte';

  interface Props {
    /** How often agents were shown the entry (above zero) and how often they found it useful. */
    counts: { shown: number; useful: number };
    /** The count as a sentence, such as "7 of 9". */
    ratio: string;
    /** The accessible name of the meter. */
    label: string;
  }

  let { counts, ratio, label }: Props = $props();
</script>

<!-- How often agents found an entry useful out of the times they were shown it: the count, and the meter that turns amber when it is low. -->
<div class="flex items-center gap-2">
  <span class="tabular min-w-[6.5ch] shrink-0 text-sm whitespace-nowrap text-fg-2">{ratio}</span>
  <UsageMeter value={counts.useful} total={counts.shown} {label} />
</div>
