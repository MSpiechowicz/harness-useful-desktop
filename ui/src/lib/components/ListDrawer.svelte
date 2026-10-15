<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade, fly } from 'svelte/transition';

  import { focusTrap } from '$lib/a11y/focus-trap';
  import { scaled } from '$lib/design/motion';

  import type { PaneDrawer } from './pane-drawer.svelte';

  interface Props {
    drawer: PaneDrawer;
    /** Accessible name of the drawer. */
    label: string;
    /** Selector of the element to focus first; defaults to the first control. */
    initialFocus?: string;
    children: Snippet;
  }

  let { drawer, label, initialFocus, children }: Props = $props();
</script>

<!-- Slides over the left of its (relatively positioned) parent, with a scrim over the rest. -->
{#if drawer.open}
  <button type="button" tabindex="-1" aria-hidden="true" class="absolute inset-0 z-30 cursor-default bg-scrim backdrop-blur-sm" transition:fade={{ duration: scaled(160) }} onclick={() => drawer.dismiss()}></button>

  <div
    role="dialog"
    aria-label={label}
    use:focusTrap={{ onEscape: () => drawer.dismiss(), initialFocus }}
    transition:fly={{ x: -32, duration: scaled(240) }}
    class="absolute inset-y-0 left-0 z-40 flex w-[min(24rem,92%)] flex-col overflow-hidden border-r border-line-strong bg-surface-1 shadow-pop outline-none"
  >
    {@render children()}
  </div>
{/if}
