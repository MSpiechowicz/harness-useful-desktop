<script lang="ts">
  import { FAMILY_NAMES, type FamilyProduct } from '$lib/brand/family';
  import Logo from '$lib/components/Logo.svelte';

  interface Props {
    product: FamilyProduct;
    /** The translated column title, read by screen readers and shown on hover. */
    title: string;
  }

  let { product, title }: Props = $props();
</script>

<!-- The title of a column with the product's mark. Each word of the product name and "MCP" get a line of their own, so
     "MCP" sits on the same line in every column whatever the name's length. -->
<span class="flex min-w-0 items-center gap-2" {title}>
  <span class="grid size-7 shrink-0 place-items-center rounded-sm border border-line bg-surface-2" aria-hidden="true"><Logo variant="mark" {product} size={13} /></span>
  <span class="sr-only">{title}</span>
  <span class="flex min-w-0 flex-col text-sm leading-4 font-medium break-words text-fg" aria-hidden="true">
    {#each FAMILY_NAMES[product].split(' ') as word, index (index)}
      <span>{word}</span>
    {/each}
    <span>MCP</span>
  </span>
</span>
