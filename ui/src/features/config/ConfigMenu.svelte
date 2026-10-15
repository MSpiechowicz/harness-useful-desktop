<script lang="ts">
  import ChevronsDownUp from '@lucide/svelte/icons/chevrons-down-up';
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
  import Copy from '@lucide/svelte/icons/copy';
  import FolderSearch from '@lucide/svelte/icons/folder-search';

  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** Accessible name of the trigger, e.g. "More actions". */
    label: string;
    /** Whether the file can be searched, folded and copied; a missing or unreadable one can only be revealed. */
    readable: boolean;
    /** Whether the file exists, so there is something to reveal. */
    exists: boolean;
    /** Whether anything in the content is open: the first item then collapses, otherwise it expands. */
    anyOpen: boolean;
    ontoggleall: () => void;
    oncopypath: () => void;
    onreveal: () => void;
  }

  let { label, readable, exists, anyOpen, ontoggleall, oncopypath, onreveal }: Props = $props();

  function choose(close: () => void, action: () => void): void {
    close();
    action();
  }
</script>

<MoreMenu {label}>
  {#snippet children({ close })}
    {#if readable}
      <MenuItem icon={anyOpen ? ChevronsDownUp : ChevronsUpDown} onclick={() => choose(close, ontoggleall)}>
        {anyOpen ? t('config.menu.collapseAll') : t('config.menu.expandAll')}
      </MenuItem>

      <MenuItem icon={Copy} onclick={() => choose(close, oncopypath)}>{t('config.menu.copyPath')}</MenuItem>
    {/if}

    <MenuItem icon={FolderSearch} disabled={!exists} onclick={() => choose(close, onreveal)}>{t('config.detail.reveal')}</MenuItem>
  {/snippet}
</MoreMenu>
