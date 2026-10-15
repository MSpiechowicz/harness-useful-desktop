<script lang="ts">
  import Info from '@lucide/svelte/icons/info';
  import type { Snippet } from 'svelte';

  import Tooltip from '$lib/components/Tooltip.svelte';

  interface Props {
    title: string;
    description: string;
    /** Important details of a longer description, one short line each, shown in the (i) tooltip after the title. */
    points?: string[];
    /** Every text `description` can switch to; the row keeps room for the longest, so switching never shifts the layout. */
    descriptions?: string[];
    control: Snippet;
    /** An icon or badge before the title. */
    leading?: Snippet;
  }

  let { title, description, points, descriptions, control, leading }: Props = $props();
</script>

<div class="flex items-center justify-between gap-6 py-4">
  <div class="flex min-w-0 items-center gap-3">
    {@render leading?.()}

    <div class="flex min-w-0 flex-col gap-0.5">
      <p class="flex items-center gap-1.5 text-base font-medium text-fg">
        {title}
        {#if points?.length}
          <Tooltip side="bottom">
            {#snippet rich()}
              <ul class="flex flex-col gap-2.5 text-fg-2">
                {#each points as point (point)}
                  <li class="relative pl-4 before:absolute before:top-[0.55em] before:left-0.5 before:size-1.5 before:rounded-full before:bg-accent">{point}</li>
                {/each}
              </ul>
            {/snippet}
            {#snippet children({ describedby })}
              <button type="button" aria-describedby={describedby} aria-label={title} class="grid size-5 place-items-center rounded-full text-fg-3 transition-colors duration-[var(--dur-fast)] hover:text-fg focus-visible:text-fg">
                <Info size={16} aria-hidden="true" />
              </button>
            {/snippet}
          </Tooltip>
        {/if}
      </p>
      {#if descriptions?.length}
        <p class="grid max-w-[700px] text-sm text-fg-2">
          {#each descriptions as text (text)}
            <span class="invisible [grid-area:1/1]" aria-hidden="true">{text}</span>
          {/each}
          <span class="[grid-area:1/1]">{description}</span>
        </p>
      {:else}
        <p class="max-w-[700px] text-sm text-fg-2">{description}</p>
      {/if}
    </div>
  </div>

  <!-- The control sits in the vertical middle of the row, whatever the height of the text beside it. -->
  <div class="shrink-0">
    {@render control()}
  </div>
</div>
