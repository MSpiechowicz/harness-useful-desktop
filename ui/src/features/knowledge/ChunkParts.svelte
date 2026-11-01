<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';

  import type { ChunkTexts } from './chunk-texts.svelte';
  import ChunkPart from './ChunkPart.svelte';

  interface Props {
    texts: ChunkTexts;
    /** The chunks one section is cut into, in order. */
    chunkIds: number[];
  }

  let { texts, chunkIds }: Props = $props();
</script>

<!-- The text of a section: one part, or each of the parts a long section is cut into. -->
<div class="flex flex-col gap-3">
  {#if chunkIds.length > 1}
    <p class="text-xs text-fg-3">{t('knowledge.chunk.overlap')}</p>
  {/if}

  {#each chunkIds as chunkId, index (chunkId)}
    <div class="flex flex-col gap-1.5">
      {#if chunkIds.length > 1}
        <p class="text-xs font-medium text-fg-3">{t('knowledge.chunk.part', { number: index + 1 })}</p>
      {/if}

      <ChunkPart {texts} {chunkId} inset />
    </div>
  {/each}
</div>
