<script lang="ts">
  import type { Snippet } from 'svelte';

  import type { IconComponent } from '$lib/components/icon';

  interface Props {
    /** The tile at the start: what kind of thing this row is. */
    icon: IconComponent;
    /** Usually a name, sometimes a button that opens a picker. */
    title: Snippet;
    /** The dim line under it: what state this is in. */
    subtitle?: string;
    /** What can be done about it, at the end of the row. */
    action?: Snippet;
  }

  let { icon: Icon, title, subtitle, action }: Props = $props();
</script>

<!-- One shape for every row of the branch menu (remote, main, conflicts): a 40px tile, two lines as tall as the tile, and what to do on the right. -->
<div class="flex items-center gap-3 px-3 py-2.5">
  <span class="grid size-10 shrink-0 place-items-center rounded-sm bg-surface-3 text-fg-2"><Icon size={18} aria-hidden="true" /></span>

  <div class={subtitle ? 'flex h-10 min-w-0 flex-1 flex-col justify-between' : 'flex h-10 min-w-0 flex-1 flex-col justify-center'}>
    <div class="h-5 truncate text-sm leading-5 text-fg">{@render title()}</div>
    {#if subtitle}<div class="h-4.5 truncate text-xs leading-4.5 text-fg-3">{subtitle}</div>{/if}
  </div>

  {#if action}{@render action()}{/if}
</div>
