<script lang="ts">
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import type { Snippet } from 'svelte';
  import { slide } from 'svelte/transition';

  import { scaled } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';
  import { uid } from '$lib/utils/ids';

  interface Props {
    /** The always-visible line that toggles the content. */
    label: string;
    open?: boolean;
    /** Small text after the label, e.g. a count. */
    meta?: string;
    class?: string;
    children: Snippet;
  }

  let { label, open = $bindable(false), meta, class: className, children }: Props = $props();

  const panelId = uid('disclosure');
</script>

<div class={className}>
  <button
    type="button"
    aria-expanded={open}
    aria-controls={panelId}
    class="flex w-full items-center gap-1.5 rounded-sm py-1 text-left text-sm font-medium text-fg-2 transition-colors duration-[var(--dur-fast)] hover:text-fg"
    onclick={() => (open = !open)}
  >
    <ChevronRight size={16} aria-hidden="true" class={cx('shrink-0 transition-transform duration-[var(--dur-fast)]', open && 'rotate-90')} />
    <span>{label}</span>
    {#if meta}<span class="tabular font-normal text-fg-3">{meta}</span>{/if}
  </button>

  <div id={panelId}>
    {#if open}
      <div transition:slide={{ duration: scaled(200) }} class="overflow-hidden">
        <div class="pt-2">{@render children()}</div>
      </div>
    {/if}
  </div>
</div>
