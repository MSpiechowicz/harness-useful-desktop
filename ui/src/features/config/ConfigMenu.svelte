<script lang="ts">
  import ChevronsDownUp from '@lucide/svelte/icons/chevrons-down-up';
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
  import Copy from '@lucide/svelte/icons/copy';
  import Ellipsis from '@lucide/svelte/icons/ellipsis';

  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** Accessible name of the trigger, e.g. "More actions". */
    label: string;
    /** Whether anything in the content is open: the first item then collapses, otherwise it expands. */
    anyOpen: boolean;
    ontoggleall: () => void;
    oncopypath: () => void;
  }

  let { label, anyOpen, ontoggleall, oncopypath }: Props = $props();

  const ITEM =
    'flex items-center gap-2 rounded-sm px-2.5 py-2 text-left text-sm text-fg transition-colors duration-[var(--dur-fast)] hover:bg-hover focus-visible:bg-hover';

  function choose(close: () => void, action: () => void): void {
    close();
    action();
  }
</script>

<Popover {label} kind="menu" align="end" class="w-48 min-w-0!">
  {#snippet trigger({ props })}
    <IconButton {...props} icon={Ellipsis} {label} />
  {/snippet}

  {#snippet children({ close })}
    <div role="none" class="flex flex-col">
      <button type="button" role="menuitem" data-menu-item class={ITEM} onclick={() => choose(close, ontoggleall)}>
        {#if anyOpen}
          <ChevronsDownUp size={ICON.sm} aria-hidden="true" />
          {t('config.menu.collapseAll')}
        {:else}
          <ChevronsUpDown size={ICON.sm} aria-hidden="true" />
          {t('config.menu.expandAll')}
        {/if}
      </button>

      <button type="button" role="menuitem" data-menu-item class={ITEM} onclick={() => choose(close, oncopypath)}>
        <Copy size={ICON.sm} aria-hidden="true" />
        {t('config.menu.copyPath')}
      </button>
    </div>
  {/snippet}
</Popover>
