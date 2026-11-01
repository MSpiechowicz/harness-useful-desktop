<script lang="ts">
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  import Badge from '$lib/components/Badge.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import type { ChunkTexts } from './chunk-texts.svelte';
  import ChunkPart from './ChunkPart.svelte';

  interface Props {
    texts: ChunkTexts;
    /** The chunks one section is cut into, in order. */
    chunkIds: number[];
  }

  let { texts, chunkIds }: Props = $props();

  /** The part on screen: one at a time, so a section cut into many parts never becomes a long scroll. */
  let index = $state(0);

  const current = $derived(chunkIds[Math.min(index, chunkIds.length - 1)]);
  const many = $derived(chunkIds.length > 1);
</script>

<!-- The text of a section: its one part, or the parts a long section is cut into, one at a time with a pager. -->
{#if many}
  <div class="flex flex-col gap-3 rounded-md border border-line px-4 pt-2.5 pb-4">
    <div class="flex items-center justify-between gap-2">
      <Badge tone="accent" size="lg">{t('knowledge.chunk.partOf', { number: index + 1, total: chunkIds.length })}</Badge>

      <div class="flex items-center gap-1">
        <IconButton icon={ChevronLeft} label={t('knowledge.chunk.previous')} disabled={index === 0} onclick={() => (index -= 1)} />
        <IconButton icon={ChevronRight} label={t('knowledge.chunk.next')} disabled={index >= chunkIds.length - 1} onclick={() => (index += 1)} />
      </div>
    </div>

    {#key current}
      <ChunkPart {texts} chunkId={current} inset />
    {/key}

    <p class="text-xs text-fg-3">{t('knowledge.chunk.overlap')}</p>
  </div>
{:else if current !== undefined}
  <ChunkPart {texts} chunkId={current} inset />
{/if}
