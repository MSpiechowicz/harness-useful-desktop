<script lang="ts">
  import type { Snippet } from 'svelte';

  import type { FamilyProduct } from '$lib/brand/family';
  import Logo from '$lib/components/Logo.svelte';

  import SetupLine from './SetupLine.svelte';
  import type { Chip } from './setup-model';

  interface Props {
    product: FamilyProduct;
    name: string;
    /** The one line under the name. */
    detail: string;
    chip: Chip;
    /** What the run has to say about the row, once it has anything. */
    children?: Snippet;
  }

  let { product, name, detail, chip, children }: Props = $props();
</script>

<!-- One member of the Harness family: mark, name, one line and the state, then whatever its run printed or failed with. Every row has this shape. -->
<li class="border-t border-line">
  <div role="group" aria-label={name} class="flex flex-col gap-2.5 py-[9px]">
    <SetupLine {lead} name={heading} {chip} />

    {@render children?.()}
  </div>
</li>

{#snippet lead()}
  <span class="grid size-control place-items-center rounded-md border border-line bg-surface-2 shadow-raised" aria-hidden="true"><Logo variant="mark" {product} size={15} /></span>
{/snippet}

{#snippet heading()}
  <p class="truncate text-sm leading-5 font-medium text-fg" title={name}>{name}</p>
  <p class="truncate text-xs leading-4 text-fg-3" title={detail}>{detail}</p>
{/snippet}
