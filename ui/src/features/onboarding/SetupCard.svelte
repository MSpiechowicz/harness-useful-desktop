<script lang="ts">
  import type { Snippet } from 'svelte';

  import type { FamilyProduct } from '$lib/brand/family';
  import Logo from '$lib/components/Logo.svelte';
  import { uid } from '$lib/utils/ids';

  import SetupDetails from './SetupDetails.svelte';
  import SetupLine from './SetupLine.svelte';
  import type { Chip } from './setup-model';

  interface Props {
    /** Whose family mark the card wears. */
    product: FamilyProduct;
    title: string;
    /** The state of the card's own item. */
    chip: Chip;
    /** What the step does, in a sentence. */
    description: string;
    /** What it changes or leaves alone, always behind the same disclosure so the cards line up. */
    details: Snippet;
    /** Quiet text after the disclosure's label, such as a count. */
    detailsMeta?: string;
  }

  let { product, title, chip, description, details, detailsMeta }: Props = $props();

  const titleId = uid('setup-card');

  let open = $state(false);
</script>

<!-- The same sections in the same order in every state: header line, description, details. -->
<section
  aria-labelledby={titleId}
  class="flex min-w-0 flex-col gap-3 rounded-lg border border-line bg-glass p-4 shadow-float [@media(max-height:720px)]:gap-2 [@media(max-height:720px)]:p-3.5"
>
  <SetupLine {lead} name={heading} {chip} />

  <p class="text-sm text-fg-2">{description}</p>

  <SetupDetails bind:open meta={detailsMeta}>{@render details()}</SetupDetails>
</section>

{#snippet lead()}
  <!-- The product's own family mark, on a tile like its app icon. -->
  <span class="grid size-control place-items-center rounded-md border border-line bg-surface-2 shadow-raised" aria-hidden="true"><Logo variant="mark" {product} size={15} /></span>
{/snippet}

{#snippet heading()}
  <h3 id={titleId} class="truncate text-lg leading-snug text-fg">{title}</h3>
{/snippet}
