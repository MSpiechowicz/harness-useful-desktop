<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    description: string;
    /** Important details of a longer description, one short line each, shown as a list under it. */
    points?: string[];
    control: Snippet;
    /** An icon or badge before the title. */
    leading?: Snippet;
  }

  let { title, description, points, control, leading }: Props = $props();
</script>

<div class="flex items-start justify-between gap-6 py-4 first:pt-0 last:pb-0">
  <div class="flex min-w-0 items-center gap-3">
    {@render leading?.()}

    <div class="flex min-w-0 flex-col gap-0.5">
      <p class="text-base font-medium text-fg">{title}</p>
      <p class="max-w-[700px] text-sm text-fg-2">{description}</p>
      {#if points?.length}
        <ul class="mt-1 flex max-w-[700px] flex-col gap-1 text-sm text-fg-2">
          {#each points as point (point)}
            <li class="relative pl-4 before:absolute before:top-[0.6em] before:left-[3px] before:size-1 before:rounded-full before:bg-fg-3">{point}</li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>

  <!-- The control's top edge lines up with the top of the title's capitals (5px into its 21px line), so the gap above
       both is the same. -->
  <div class="shrink-0 pt-[5px]">
    {@render control()}
  </div>
</div>
