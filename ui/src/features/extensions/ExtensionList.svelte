<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import { cx } from '$lib/utils/cx';

  import type { ExtItem } from './extension-items';

  interface Props {
    items: ExtItem[];
    selectedId: string;
    label: string;
    onselect: (id: string) => void;
  }

  let { items, selectedId, label, onselect }: Props = $props();

  /** Above this many rows the list is virtualized with a fixed row height. */
  const VIRTUALIZE_ABOVE = 100;
  const ROW_HEIGHT = 84;
</script>

<!-- The same 12px gutter as the search field above on both sides: the rows line up with it and stay clear of the scrollbar. -->
{#if items.length > VIRTUALIZE_ABOVE}
  <VirtualList {items} estimate={ROW_HEIGHT} {label} key={(entry) => entry.id} class="min-h-0 flex-1 px-3">
    {#snippet row(entry)}
      {@render rowButton(entry, true)}
    {/snippet}
  </VirtualList>
{:else}
  <ul aria-label={label} class="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-3 pb-3">
    {#each items as entry (entry.id)}
      <li>{@render rowButton(entry, false)}</li>
    {/each}
  </ul>
{/if}

{#snippet rowButton(entry: ExtItem, fixed: boolean)}
  {@const selected = entry.id === selectedId}
  <button
    type="button"
    aria-current={selected ? 'true' : undefined}
    onclick={() => onselect(entry.id)}
    class={cx(
      // The focus ring lies on the row's own border (not beside it), so there is one line, never two, and the
      // scrolling list cannot clip it.
      'flex w-full flex-col justify-center gap-1 rounded-md border px-3 py-2 text-left transition-colors duration-[var(--dur-fast)] focus-visible:outline-offset-[-1px]',
      fixed && 'h-[80px]',
      selected ? 'border-accent/30 bg-accent-wash' : 'border-transparent hover:bg-hover active:bg-press',
    )}
  >
    <span class="truncate text-base font-medium text-fg">{entry.title}</span>
    {#if entry.subtitle}
      <span class="truncate text-xs text-fg-2">{entry.subtitle}</span>
    {/if}
    <span class="flex gap-1.5 overflow-hidden">
      {#each entry.badges.slice(0, 3) as badge (badge.label)}
        <Badge tone={badge.tone}>{badge.label}</Badge>
      {/each}
    </span>
  </button>
{/snippet}
