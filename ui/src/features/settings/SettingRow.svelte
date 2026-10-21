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
    /** A note under the control (a hint about its value). It adds height under the row and never moves the control off the middle of the title and description. */
    below?: Snippet;
    /** An icon or badge before the title. */
    leading?: Snippet;
  }

  let { title, description, points, descriptions, control, below, leading }: Props = $props();
</script>

<div class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-x-6 py-4">
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

  <!-- The control (a switch, a button, a segmented control) is centred on the title and description beside it, whatever the height of the text. A flex box, so no line box adds space around an inline control. -->
  <div class="flex items-center justify-self-end">
    {@render control()}
  </div>

  {#if below}
    <div class="col-start-2">
      {@render below()}
    </div>
  {/if}
</div>
