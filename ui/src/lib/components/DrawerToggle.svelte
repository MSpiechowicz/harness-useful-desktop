<script lang="ts">
  import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';

  import Button from './Button.svelte';
  import { ICON } from './icon';
  import type { PaneDrawer } from './pane-drawer.svelte';

  interface Props {
    drawer: PaneDrawer;
    /** What the drawer holds, e.g. "Sessions". */
    label: string;
    /** How many items it lists. */
    count?: number;
  }

  let { drawer, label, count }: Props = $props();
</script>

<!-- Shows nothing while the list is a column; as a drawer, the icon tells what the next click does. -->
{#if drawer.active}
  <Button data-drawer-toggle aria-haspopup="dialog" aria-expanded={drawer.open} onclick={() => drawer.toggle()}>
    {#snippet leading()}
      {#if drawer.open}<PanelLeftClose size={ICON.sm} aria-hidden="true" />{:else}<PanelLeftOpen size={ICON.sm} aria-hidden="true" />{/if}
    {/snippet}
    {label}
    {#if count !== undefined}<span class="tabular rounded-full bg-surface-3 px-1.5 text-xs leading-5 text-fg-2">{count}</span>{/if}
  </Button>
{/if}
