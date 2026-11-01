<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';

  import type { ChunkTexts } from './chunk-texts.svelte';
  import Badge from '$lib/components/Badge.svelte';

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
    <!-- Each part of a cut section is its own framed box with a label above its text, so where one ends and the next begins is plain. -->
    <div class={chunkIds.length > 1 ? 'flex flex-col gap-2 rounded-md border border-line px-3 pt-2 pb-2.5' : 'contents'}>
      {#if chunkIds.length > 1}
        <span class="self-start"><Badge tone="accent">{t('knowledge.chunk.part', { number: index + 1 })}</Badge></span>
      {/if}

      <ChunkPart {texts} {chunkId} inset />
    </div>
  {/each}
</div>
