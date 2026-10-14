<script lang="ts">
  import Ellipsis from '@lucide/svelte/icons/ellipsis';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  import { ICON } from '$lib/components/icon';
  import IconButton from '$lib/components/IconButton.svelte';
  import Popover from '$lib/components/Popover.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  interface Props {
    /** Accessible name of the trigger, e.g. "Session actions". */
    label: string;
    open?: boolean;
    /** Classes of the trigger button. */
    class?: string;
    ondelete: () => void;
  }

  let { label, open = $bindable(false), class: className, ondelete }: Props = $props();

  function remove(close: () => void): void {
    close();
    ondelete();
  }
</script>

<Popover {label} kind="menu" align="end" bind:open class="w-44 min-w-0!">
  {#snippet trigger({ props })}
    <IconButton {...props} icon={Ellipsis} {label} class={className} />
  {/snippet}

  {#snippet children({ close })}
    <div role="none" class="flex flex-col">
      <button
        type="button"
        role="menuitem"
        data-menu-item
        class="flex items-center gap-2 rounded-sm px-2.5 py-2 text-left text-sm text-danger transition-colors hover:bg-danger-wash focus-visible:bg-danger-wash"
        onclick={() => remove(close)}
      >
        <Trash2 size={ICON.sm} aria-hidden="true" />
        {t('sessions.menu.delete')}
      </button>
    </div>
  {/snippet}
</Popover>
