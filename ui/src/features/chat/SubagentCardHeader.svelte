<script lang="ts">
  import type { Snippet } from 'svelte';

  import CopyButton from '$lib/components/CopyButton.svelte';
  import IconTile from '$lib/components/IconTile.svelte';

  interface Props {
    tone?: 'plain' | 'success';
    /** The glyph in the tile. */
    icon: Snippet;
    /** The first line: what the card holds. */
    title: Snippet;
    /** The second line, when there is one. */
    sub?: Snippet;
    copyText: string;
    copyLabel: string;
  }

  let { tone = 'plain', icon, title, sub, copyText, copyLabel }: Props = $props();
</script>

<!-- One header for every card of the panel (brief, output, report): the same padding, and a text block two caption lines tall
     whether or not the second line is there, so switching tabs never changes the header's height. -->
<header class="flex shrink-0 items-center gap-3.5 border-b border-line py-3.5 pr-2 pl-3.5">
  <IconTile size="md" {tone}>{@render icon()}</IconTile>
  <div class="flex min-h-[calc(2*var(--caption-line))] min-w-0 flex-1 flex-col justify-center text-xs leading-[var(--caption-line)] text-fg-3">
    <span class="flex min-w-0 items-center gap-2">{@render title()}</span>
    {#if sub}<span class="truncate">{@render sub()}</span>{/if}
  </div>
  <CopyButton control text={copyText} label={copyLabel} />
</header>
