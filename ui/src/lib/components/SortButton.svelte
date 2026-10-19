<script lang="ts">
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';

  import { cx } from '$lib/utils/cx';

  import { ICON } from './icon';

  interface Props {
    label: string;
    /** The table is sorted by this column. */
    active: boolean;
    /** The sort runs from the biggest down; only read while the column is active. */
    desc: boolean;
    /** The text of the column is set to the right edge. */
    end?: boolean;
    onclick: () => void;
  }

  let { label, active, desc, end = false, onclick }: Props = $props();
</script>

{#snippet arrow()}
  {#if active}
    <span class="grid size-4 shrink-0 place-items-center" aria-hidden="true">
      {#if desc}<ArrowDown size={ICON.sm} />{:else}<ArrowUp size={ICON.sm} />{/if}
    </span>
  {/if}
{/snippet}

<!-- The header of a sortable column: a button, the same in every table. The arrow goes on the side away from the edge the text is set to, so the label stays where it is when a sort turns on. -->
<button
  type="button"
  class={cx(
    '-mx-2 inline-flex h-7 max-w-full items-center gap-1 rounded-sm px-2 uppercase transition-colors duration-[var(--dur-fast)] hover:bg-hover hover:text-fg-2 focus-visible:outline-2',
    active && 'text-fg',
  )}
  {onclick}
>
  {#if end}{@render arrow()}{/if}
  <span class="truncate">{label}</span>
  {#if !end}{@render arrow()}{/if}
</button>
