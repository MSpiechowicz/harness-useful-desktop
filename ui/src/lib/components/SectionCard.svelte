<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  import Card from './Card.svelte';
  import type { SectionHead } from './section-card';

  interface Props extends SectionHead {
    class?: string;
    children: Snippet;
  }

  let { heading, hint, flat = false, class: className, children }: Props = $props();
</script>

{#snippet content()}
  <!-- One gap from the heading (or the hint under it) to the content, in every section: the 14px a Details row starts at. -->
  <h3 class={cx('text-xs font-medium tracking-wider text-fg-3 uppercase', hint ? 'mb-1' : 'mb-3.5')}>{heading}</h3>
  {#if hint}<p class="mb-3.5 text-sm text-fg-3">{hint}</p>{/if}
  {@render children()}
{/snippet}

<!-- A titled card of a detail pane: the small caps heading, then the section's content. Flat, the card is only a hairline
     above it (none above the first section). -->
{#if flat}
  <section class={cx('border-t border-line pt-4 first:border-t-0 first:pt-0', className)}>
    {@render content()}
  </section>
{:else}
  <Card as="section" padded={false} class={cx('p-3.5', className)}>
    {@render content()}
  </Card>
{/if}
