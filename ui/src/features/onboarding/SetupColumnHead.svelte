<script lang="ts">
  import type { FamilyProduct } from '$lib/brand/family';
  import Logo from '$lib/components/Logo.svelte';

  interface Props {
    product: FamilyProduct;
    title: string;
  }

  let { product, title }: Props = $props();
</script>

<!-- The title of a column with the product's mark; a long title wraps onto more lines. -->
<span class="flex min-w-0 items-center gap-2">
  <span class="grid size-7 shrink-0 place-items-center rounded-sm border border-line bg-surface-2" aria-hidden="true"><Logo variant="mark" {product} size={13} /></span>
  <span class="line-clamp-3 text-sm leading-4 font-medium break-words text-fg" {title}>{title}</span>
</span>
