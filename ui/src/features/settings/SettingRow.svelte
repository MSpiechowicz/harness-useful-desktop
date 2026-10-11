<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    description: string;
    control: Snippet;
    /** An icon or badge before the title. */
    leading?: Snippet;
  }

  let { title, description, control, leading }: Props = $props();
</script>

<div class="flex items-start justify-between gap-6 py-4 first:pt-0 last:pb-0">
  <div class="flex min-w-0 items-center gap-3">
    {@render leading?.()}

    <div class="flex min-w-0 flex-col gap-0.5">
      <p class="text-base font-medium text-fg">{title}</p>
      <p class="text-sm text-fg-2">{description}</p>
    </div>
  </div>

  <!-- The control's top edge lines up with the top of the title's capitals (5px into its 21px line), so the gap above
       both is the same. -->
  <div class="shrink-0 pt-[5px]">
    {@render control()}
  </div>
</div>
