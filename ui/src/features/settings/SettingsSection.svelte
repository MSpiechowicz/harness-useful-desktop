<script lang="ts">
  import type { Snippet } from 'svelte';

  import Card from '$lib/components/Card.svelte';
  import { uid } from '$lib/utils/ids';

  interface Props {
    title: string;
    /** Anchor for the section nav; scrolled to from there and from the command palette. */
    id?: string;
    children: Snippet;
  }

  let { title, id, children }: Props = $props();

  const headingId = uid('settings-heading');
</script>

<section {id} aria-labelledby={headingId} class="flex scroll-mt-6 flex-col gap-3" data-settings-section={id}>
  <h2 id={headingId} class="px-1 text-sm font-medium tracking-wider text-fg-3 uppercase">{title}</h2>
  <!-- No padding of its own: every row has the same 14px above and below, so a row's control is centred between the card's edge or the divider above and the divider or edge below. -->
  <Card elevation={1} padded={false} class="divide-y divide-line px-5">
    {@render children()}
  </Card>
</section>
