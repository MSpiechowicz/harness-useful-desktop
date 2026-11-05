<script lang="ts">
  import { untrack } from 'svelte';

  import { ICON } from '$lib/components/icon';
  import LoadingSpinner from '$lib/components/LoadingSpinner.svelte';
  import { LOADING_DELAY_MS } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeId } from '$lib/ipc/knowledge-id';

  import type { ChunkTexts } from './chunk-texts.svelte';
  import ChunkPart from './ChunkPart.svelte';
  import PartTrack from './PartTrack.svelte';

  interface Props {
    texts: ChunkTexts;
    /** The chunks one section is cut into, in order. */
    chunkIds: KnowledgeId[];
    /** The part on screen, kept by the list so a row that scrolls off a page comes back on the same part. */
    index?: number;
  }

  let { texts, chunkIds, index = $bindable(0) }: Props = $props();

  const current = $derived(chunkIds[Math.min(index, chunkIds.length - 1)]);

  /** The part whose text is on screen. It stays until the next part is read, so the block never collapses in between. */
  let shown = $state(untrack(() => current));

  const waiting = $derived(shown !== current);

  // Read once per part: this must not follow the status, or a part that fails would be read again for ever.
  $effect(() => {
    const id = current;

    untrack(() => void texts.show(id));
  });

  $effect(() => {
    const status = texts.stateOf(current)?.status;

    if (status === 'ready' || status === 'error') {
      shown = current;
    }
  });
</script>

<!-- The text of a section, one part at a time, so a section cut into many parts never becomes a long scroll. -->
{#if chunkIds.length > 1}
  <PartTrack index={Math.min(index, chunkIds.length - 1)} total={chunkIds.length} onpick={(part) => (index = part)} />
{/if}

{#if shown !== undefined}
  <!-- A quick read swaps the text in place. A slow one fades it out and shows a small spinner in the same space. -->
  <div class="relative">
    <div class={waiting ? 'part-waiting keep-motion' : ''} style:animation-delay={waiting ? `${LOADING_DELAY_MS}ms` : undefined}>
      <ChunkPart {texts} chunkId={shown} inset />
    </div>

    {#if waiting}
      <LoadingSpinner size={ICON.sm} class="absolute inset-0" label={t('knowledge.chunk.loading')} />
    {/if}
  </div>
{/if}

<style>
  .part-waiting {
    animation: part-fade var(--dur-base) var(--ease-in-out) both;
  }

  @keyframes part-fade {
    to {
      opacity: 0;
    }
  }
</style>
