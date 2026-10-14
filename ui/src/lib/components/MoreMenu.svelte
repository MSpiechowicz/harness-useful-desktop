<script lang="ts">
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import type { Snippet } from 'svelte';

  import IconButton from './IconButton.svelte';
  import Popover from './Popover.svelte';

  interface Props {
    /** Accessible name of the "⋯" trigger and of the menu, e.g. "More actions". */
    label: string;
    open?: boolean;
    /** Classes of the trigger button. */
    class?: string;
    /** Classes of the menu panel, mostly its width. */
    panelClass?: string;
    /** The `MenuItem`s; `close` shuts the menu. */
    children: Snippet<[{ close: () => void }]>;
  }

  let { label, open = $bindable(false), class: className, panelClass = 'w-48', children: items }: Props = $props();
</script>

<Popover {label} kind="menu" align="end" bind:open class={`${panelClass} min-w-0!`}>
  {#snippet trigger({ props })}
    <IconButton {...props} icon={Ellipsis} {label} class={className} />
  {/snippet}

  {#snippet children({ close })}
    <div role="none" class="flex flex-col">
      {@render items({ close })}
    </div>
  {/snippet}
</Popover>
