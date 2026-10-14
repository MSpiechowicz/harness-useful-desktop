<script lang="ts">
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import MenuItem from '$lib/components/MenuItem.svelte';
  import MoreMenu from '$lib/components/MoreMenu.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  type Props = {
    /** Accessible name of the trigger, e.g. "Session actions". */
    label: string;
    /** Whether the menu is open; the row keeps its buttons visible while it is. */
    open?: boolean;
    ondelete: () => void;
  };

  let { label, open = $bindable(false), ondelete }: Props = $props();

  function remove(close: () => void): void {
    close();
    ondelete();
  }
</script>

<MoreMenu {label} bind:open panelClass="w-44">
  {#snippet children({ close })}
    <MenuItem tone="danger" icon={Trash2} onclick={() => remove(close)}>{t('sessions.menu.delete')}</MenuItem>
  {/snippet}
</MoreMenu>
