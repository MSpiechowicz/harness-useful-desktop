<script lang="ts">
  import type { Snippet } from 'svelte';

  import Card from '$lib/components/Card.svelte';

  interface Props {
    title: string;
    children: Snippet;
  }

  let { title, children }: Props = $props();
</script>

<section class="flex flex-col gap-3">
  <h2 class="px-1 text-sm font-medium tracking-wider text-fg-3 uppercase">{title}</h2>
  <Card elevation={1} class="divide-y divide-line">
    {@render children()}
  </Card>
</section>
