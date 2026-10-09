<script lang="ts">
  import type { Snippet } from 'svelte';

  import type { IconComponent } from './icon';

  interface Props {
    icon: IconComponent;
    title: string;
    body?: string;
    action?: Snippet;
  }

  let { icon: Icon, title, body, action }: Props = $props();
</script>

<div class="flex flex-col items-center gap-5 px-8 py-14 text-center">
  <div class="relative grid size-16 place-items-center rounded-lg border border-line-strong bg-surface-2 shadow-float">
    <span class="absolute inset-0 -z-10 rounded-lg opacity-50 blur-xl" style:background="var(--brand-gradient)" aria-hidden="true"></span>
    <Icon class="text-accent-text" size={28} strokeWidth={1.6} aria-hidden="true" />
  </div>

  <div class="flex max-w-md flex-col gap-1.5">
    <h2 class="text-xl text-fg">{title}</h2>
    {#if body}
      <p class="text-base text-fg-2">{body}</p>
    {/if}
  </div>

  {#if action}
    {@render action()}
  {/if}
</div>
