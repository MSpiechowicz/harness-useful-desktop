<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HarnessSummary } from '$lib/ipc/bindings/HarnessSummary';
  import { cx } from '$lib/utils/cx';

  import { featureTags } from './harness-tags';

  interface Props {
    summary: HarnessSummary;
  }

  let { summary }: Props = $props();

  const tags = $derived(featureTags(summary.capabilities));
  /** Introduced like the others, greyed out: the app cannot chat with it yet (the picker says so). */
  const soon = $derived(!summary.capabilities.chat);
</script>

<!--
  Three fixed rows, in this order, whatever the harness: header, description and tags. The card is a subgrid of the
  step's grid, so a row is as tall as the tallest of its siblings and the descriptions and the tags of neighbouring cards
  start on the same line, however many lines the wording takes.
-->
<article
  aria-label={summary.info.name}
  class={cx('row-span-3 grid grid-rows-subgrid gap-y-4 rounded-lg border-card border-line bg-glass p-6 shadow-float [@media(max-height:720px)]:gap-y-3 [@media(max-height:720px)]:p-4', soon && 'opacity-60 grayscale')}
>
  <header class="flex items-center gap-3">
    <HarnessBadge info={summary.info} size="lg" />
    <h3 class="min-w-0 flex-1 text-xl text-fg">{summary.info.name}</h3>
  </header>

  <p class="text-sm text-fg-2">{summary.info.description}</p>

  <ul class="flex flex-wrap content-start items-start gap-2" aria-label={t('onboarding.meet.tags')}>
    {#each tags as tag (tag)}
      <li><Badge size="lg">{t(tag)}</Badge></li>
    {/each}
  </ul>
</article>
