<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  import type { MenuSearchTexts } from './menu-search';

  interface Props {
    /** The texts of the search field. Without them the menu has no field and the list is as tall as its choices. */
    search?: MenuSearchTexts;
    /** What is typed in the field. */
    needle: string;
    /** Nothing matches what is typed: the list says so. */
    none: boolean;
    /** The choices. */
    children: Snippet;
  }

  let { search, needle = $bindable(), none, children }: Props = $props();

  // The panel is built anew each time the menu opens, so what was typed before (and left by Escape or a click outside) does not narrow the next one.
  $effect(() => {
    needle = '';
  });
</script>

<!-- The part of a menu with many choices that the filters and the form dropdowns share: a field above the choices, which is where focus lands on open
     (the arrow keys go on to the choices), and a list of one fixed height with its own scroll, so the menu keeps its size while typing narrows it. -->
{#if search}
  <div class="pb-1.5">
    <input
      type="search"
      data-autofocus
      bind:value={needle}
      aria-label={search.label}
      placeholder={search.placeholder}
      autocomplete="off"
      spellcheck="false"
      class="h-control w-full rounded-md border border-line-strong bg-surface-2 px-2.5 text-sm text-fg outline-none placeholder:text-fg-3 focus-visible:border-accent/50"
    />
  </div>
{/if}

<div role="none" class={cx('flex flex-col', search && 'h-[var(--menu-list-h)] overflow-y-auto')}>
  {#if search && none}
    <p class="px-2.5 py-2 text-sm text-fg-3" role="status">{search.empty}</p>
  {/if}

  {@render children()}
</div>
