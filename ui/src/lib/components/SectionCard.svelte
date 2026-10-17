<script lang="ts">
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  import Card from './Card.svelte';

  interface Props {
    heading: string;
    /** No card: the section is told apart by a hairline above it and its small caps heading, so a pane of sections is one flat page. */
    flat?: boolean;
    class?: string;
    children: Snippet;
  }

  let { heading, flat = false, class: className, children }: Props = $props();
</script>

{#snippet content()}
  <h3 class="mb-2.5 text-xs font-medium tracking-wider text-fg-3 uppercase">{heading}</h3>
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
