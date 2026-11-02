<script lang="ts">
  import Upload from '@lucide/svelte/icons/upload';
  import { fade } from 'svelte/transition';

  import type { IconComponent } from '$lib/components/icon';
  import { DURATION, rise, scaled } from '$lib/design/motion';

  interface Props {
    /** A drag is over the area this covers (the closest positioned parent). */
    shown: boolean;
    title: string;
    hint: string;
    icon?: IconComponent;
  }

  let { shown, title, hint, icon: Icon = Upload }: Props = $props();
</script>

<!-- Covers its parent while a file is dragged over it, and says what a drop does. It takes no pointer events, so the drag keeps reaching what is beneath. -->
{#if shown}
  <div in:fade={{ duration: scaled(DURATION.base) }} out:fade={{ duration: scaled(DURATION.fast) }} class="pointer-events-none absolute inset-0 z-30 grid place-items-center bg-surface-1/85 p-6 backdrop-blur-sm" aria-hidden="true">
    <div in:rise={{ y: 10, duration: 280 }} class="flex size-full flex-col items-center justify-center gap-4 rounded-xl border-2 border-dashed border-accent/50 bg-accent-wash px-4 text-center">
      <span class="grid size-16 place-items-center rounded-lg border border-line-strong bg-surface-2 text-accent-text shadow-float"><Icon size={28} strokeWidth={1.6} /></span>

      <div class="flex flex-col gap-1">
        <p class="text-xl text-fg">{title}</p>
        <p class="text-base text-fg-2">{hint}</p>
      </div>
    </div>
  </div>
{/if}
