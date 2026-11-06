<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { formatCount } from '$lib/utils/format';

  interface Props {
    /** The one-based question on screen. */
    number: number;
    total: number;
  }

  let { number, total }: Props = $props();
</script>

<!-- Where this question sits among the others, in the look of the Knowledge part track. It only shows the place. -->
<div class="mb-5 flex items-center gap-2" data-question-progress>
  <span class="tabular shrink-0 text-xs text-fg-2">{t('chat.request.questionOf', { number: formatCount(number), total: formatCount(total) })}</span>

  <div class="flex min-w-0 flex-1 gap-[var(--part-segment-gap)]" aria-hidden="true">
    {#each { length: total }, part (part)}
      <span class="h-[var(--part-track-h)] min-w-0 flex-1 rounded-full {part + 1 === number ? 'bg-accent-text' : 'bg-fg-3/40'}"></span>
    {/each}
  </div>
</div>
