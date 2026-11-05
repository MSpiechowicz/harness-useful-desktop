<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import type { Component, Snippet } from 'svelte';
  import { slide } from 'svelte/transition';

  import Markdown from '$lib/components/Markdown.svelte';
  import { scaled } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';

  interface Props {
    icon: Component<{ size?: number; 'aria-hidden'?: 'true' }>;
    label: string;
    /** Markdown the agent wrote, or plain words of the app. */
    value?: string;
    /** Reads a step quieter: nothing was used. */
    quiet?: boolean;
    /** The summary of a row that opens to `tree`. */
    summary?: string;
    tree?: Snippet;
  }

  let { icon: Icon, label, value = '', quiet = false, summary, tree }: Props = $props();

  let open = $state(true);

  const GRID = 'grid gap-x-3 pr-2 pl-[calc(18px-var(--spacing)*1.5)]';
</script>

<!-- On the grid of the card's header (`ChatRow`): the inset is its 18px less the 1.5 of the list around the rows, the icon box is its
     tile, and the label starts under its title. The icon shares the box of one line of text, so it stays centred on the first
     line whatever wraps under it. -->
<div class={cx(GRID, 'grid-cols-[var(--tile-md)_var(--status-label-w)_minmax(0,1fr)] rounded-sm py-2 text-base leading-5 transition-colors duration-[var(--dur-fast)] hover:bg-hover')}>
  <span class={cx('flex h-5 w-7 items-center justify-center', quiet ? 'text-fg-3' : 'text-accent-text')}><Icon size={16} aria-hidden="true" /></span>
  <span class={quiet ? 'text-fg-3' : 'text-fg-2'}>{label}</span>

  {#if summary !== undefined}
    <button type="button" class="inline-flex items-center gap-1.5 justify-self-start text-fg" aria-expanded={open} onclick={() => (open = !open)}>
      {summary}
      <ChevronRight size={14} aria-hidden="true" class={cx('text-fg-3 transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} />
    </button>
  {:else if quiet}
    <span class="text-fg-3">{value}</span>
  {:else}
    <div class="min-w-0 [&_.md]:leading-5!"><Markdown source={value} /></div>
  {/if}
</div>

{#if tree && open}
  <div transition:slide={{ duration: scaled(180) }} class={cx(GRID, 'grid-cols-[var(--tile-md)_minmax(0,1fr)] overflow-hidden pt-0.5 pb-2.5')}>
    <div class="col-start-2 min-w-0">{@render tree()}</div>
  </div>
{/if}
