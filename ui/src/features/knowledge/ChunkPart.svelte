<script lang="ts">
  import { untrack } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import type { ChunkTexts } from './chunk-texts.svelte';
  import { blocksOf, highlightPieces } from './knowledge-text';

  interface Props {
    texts: ChunkTexts;
    chunkId: number;
    /** The words of the search to mark in the text. */
    query?: string;
    /** Inside an open row card: the card is the box, so the text has none of its own. */
    inset?: boolean;
  }

  let { texts, chunkId, query = '', inset = false }: Props = $props();

  const chunk = $derived(texts.stateOf(chunkId));

  // The text is read when the part is first shown, not with the outline.
  $effect(() => {
    const id = chunkId;

    untrack(() => void texts.load(id));
  });
</script>

{#snippet marked(text: string)}
  {#each highlightPieces(text, query) as piece, index (index)}{#if piece.match}<mark class="rounded-sm bg-accent-wash text-fg">{piece.text}</mark>{:else}{piece.text}{/if}{/each}
{/snippet}

{#if chunk?.status === 'ready'}
  {@const blocks = blocksOf(chunk.text)}

  <div class="flex max-h-72 flex-col gap-2 overflow-y-auto text-sm text-fg-2 {inset ? '' : 'rounded-md bg-surface-2 px-3 py-2'}">
    {#each blocks as block, index (index)}
      {#if block.kind === 'code'}
        <pre class="overflow-x-auto rounded-sm bg-surface-3 px-2 py-1.5 font-mono text-xs text-fg">{@render marked(block.text)}</pre>
      {:else}
        <p class="whitespace-pre-wrap">{@render marked(block.text)}</p>
      {/if}
    {:else}
      <p class="text-fg-3">{t('knowledge.chunk.empty')}</p>
    {/each}
  </div>
{:else if chunk?.status === 'error'}
  <div class="flex flex-col items-start gap-3">
    <p class="text-sm text-danger" role="alert">{chunk.message}</p>
    <Button onclick={() => void texts.load(chunkId)}>{t('memory.tryAgain')}</Button>
  </div>
{:else}
  <div class="flex flex-col gap-2" role="status" aria-label={t('knowledge.chunk.loading')}>
    <Skeleton height="1rem" width="60%" />
    <Skeleton height="1rem" />
    <Skeleton height="1rem" width="80%" />
  </div>
{/if}
