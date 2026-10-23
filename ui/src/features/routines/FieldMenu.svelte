<script lang="ts">
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import type { Snippet } from 'svelte';

  import { SELECT_TRIGGER } from '$lib/components/docs/doc-field';
  import { ICON } from '$lib/components/icon';
  import Popover from '$lib/components/Popover.svelte';

  interface Props {
    /** Names the menu. */
    label: string;
    /** `dialog` for a menu with a field to type in, `menu` for plain choices. */
    kind?: 'menu' | 'dialog';
    /** The id of the visible label that names the field. */
    labelledby: string;
    open?: boolean;
    /** What the field shows: the choice made. */
    value: Snippet;
    /** The menu, given a way to close it once a choice is made. */
    choices: Snippet<[{ close: () => void }]>;
  }

  let { label, kind = 'menu', labelledby, open = $bindable(false), value, choices }: Props = $props();
</script>

<!-- A choice from a list that is asked for when the menu opens (the models of a harness): the field of `DocSelect`, with the menu of the chat's pickers. -->
<Popover {label} {kind} side="auto" rootClass="w-full" class="w-full min-w-0!" bind:open>
  {#snippet trigger({ props })}
    <button {...props} type="button" aria-labelledby={labelledby} class={SELECT_TRIGGER}>
      {@render value()}
      <ChevronDown size={ICON.sm} class="shrink-0 text-fg-3" aria-hidden="true" />
    </button>
  {/snippet}

  {#snippet children({ close })}
    {@render choices({ close })}
  {/snippet}
</Popover>
