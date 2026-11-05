<script lang="ts">
  import Spinner from '$lib/components/Spinner.svelte';
  import { LOADING_DELAY_MS } from '$lib/design/motion';
  import { cx } from '$lib/utils/cx';

  interface Props {
    /** What is being read, for screen readers: "Loading hooks". */
    label: string;
    /** How long it stays invisible. A quick read swaps in without anything blinking. */
    delay?: number;
    /** The space it fills, when the parent has no height to fill. */
    class?: string;
  }

  let { label, delay = LOADING_DELAY_MS, class: className = 'size-full' }: Props = $props();
</script>

<!-- One spinner in the middle of a view or a pane while its content is read. A CSS delay, not a transition: a view that opens already
     loading renders this in its first pass, where a local Svelte transition would not play. `keep-motion` keeps that delay when motion is off. -->
<div class={cx('loading-spinner keep-motion grid min-h-0 place-items-center', className)} style:animation-delay="{delay}ms">
  <Spinner size={32} {label} class="text-fg-3" />
</div>

<style>
  .loading-spinner {
    animation: enter-fade var(--dur-base) var(--ease-in-out) both;
  }

  :global(:root[data-motion='off']) .loading-spinner {
    animation-duration: 0.001ms;
  }
</style>
