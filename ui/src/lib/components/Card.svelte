<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  interface Props {
    elevation?: 1 | 2 | 3;
    /** Lifts on hover; use for cards that are entirely clickable. */
    interactive?: boolean;
    padded?: boolean;
    as?: 'div' | 'section' | 'article';
    class?: string;
    children: Snippet;
  }

  let { elevation = 1, interactive = false, padded = true, as = 'div', class: className, children }: Props = $props();

  const SHADOWS = { 1: 'shadow-raised', 2: 'shadow-float', 3: 'shadow-pop' } as const;
</script>

<svelte:element
  this={as}
  class={cx(
    'rounded-lg border-card border-line bg-surface-1',
    SHADOWS[elevation],
    padded && 'p-5',
    interactive &&
      'transition-[transform,box-shadow] duration-[var(--dur-base)] ease-[var(--ease-out-expo)] hover:-translate-y-0.5 hover:shadow-float',
    className,
  )}
>
  {@render children()}
</svelte:element>
