<script lang="ts">
  import type { Snippet } from 'svelte';

  import type { LimitView } from '$lib/dashboard/limit-dial';

  interface Props {
    view: LimitView;
    class?: string;
    children: Snippet;
  }

  let { view, class: className, children }: Props = $props();
</script>

<!-- One limit as a meter: the label says what it is, how much, how it is heading and when it resets, so the drawing and the
     text around it are only pictures and repeat nothing for a screen reader. -->
<div role="meter" aria-label={view.label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={view.percent} aria-valuetext={view.reading} class={className}>
  {@render children()}
</div>
