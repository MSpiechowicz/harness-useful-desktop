<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    description: string;
    control: Snippet;
  }

  let { title, description, control }: Props = $props();
</script>

<div class="flex items-center justify-between gap-6 py-4 first:pt-0 last:pb-0">
  <div class="flex min-w-0 flex-col gap-0.5">
    <p class="text-base font-medium text-fg">{title}</p>
    <p class="text-sm text-fg-2">{description}</p>
  </div>

  <div class="shrink-0">
    {@render control()}
  </div>
</div>
