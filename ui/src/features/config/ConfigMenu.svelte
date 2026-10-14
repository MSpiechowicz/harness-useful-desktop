<script lang="ts">
  import ChevronsDownUp from '@lucide/svelte/icons/chevrons-down-up';
  import ChevronsUpDown from '@lucide/svelte/icons/chevrons-up-down';
  import Copy from '@lucide/svelte/icons/copy';

  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
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

  function choose(close: () => void, action: () => void): void {
    close();
    action();
  }
</script>

<MoreMenu {label}>
  {#snippet children({ close })}
    <MenuItem icon={anyOpen ? ChevronsDownUp : ChevronsUpDown} onclick={() => choose(close, ontoggleall)}>
      {anyOpen ? t('config.menu.collapseAll') : t('config.menu.expandAll')}
    </MenuItem>

    <MenuItem icon={Copy} onclick={() => choose(close, oncopypath)}>{t('config.menu.copyPath')}</MenuItem>
  {/snippet}
</MoreMenu>
