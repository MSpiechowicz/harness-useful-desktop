<script lang="ts">
  import { highlightSegments } from '$lib/utils/fuzzy';

  interface Props {
    text: string;
    indices: number[];
  }

  let { text, indices }: Props = $props();

  const segments = $derived(highlightSegments(text, indices));
</script>

<!-- No whitespace between the blocks: each segment must render flush against the next. -->
{#each segments as segment, index (index)}{#if segment.match}<mark class="bg-transparent font-semibold text-accent-text">{segment.text}</mark>{:else}{segment.text}{/if}{/each}
