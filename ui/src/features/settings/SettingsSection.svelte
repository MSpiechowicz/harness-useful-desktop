<script lang="ts">
  import type { Snippet } from 'svelte';

  import Card from '$lib/components/Card.svelte';
  import { uid } from '$lib/utils/ids';

  interface Props {
    title: string;
    /** Anchor for the section nav; scrolled to from there and from the command palette. */
    id?: string;
    /** Between the heading and the card: a banner about the whole section. */
    above?: Snippet;
    children: Snippet;
    /** After the card, still inside the section: more headings and cards that belong to it, found by its search and nav entry. */
    after?: Snippet;
  }

  let { title, id, above, children, after }: Props = $props();

  const headingId = uid('settings-heading');
</script>

<section {id} aria-labelledby={headingId} class="flex scroll-mt-6 flex-col gap-3" data-settings-section={id}>
  <h2 id={headingId} class="px-1 text-sm font-medium tracking-wider text-fg-3 uppercase">{title}</h2>
  {@render above?.()}
  <!-- No padding of its own: every row has the same 14px above and below, so a row's control is centred between the card's edge or the divider above and the divider or edge below. -->
  <Card elevation={1} padded={false} class="divide-y divide-line px-5">
    {@render children()}
  </Card>
  {@render after?.()}
</section>
