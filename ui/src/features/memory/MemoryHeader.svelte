<script lang="ts">
  import type { Snippet } from 'svelte';

  import ArrowLeftRight from '@lucide/svelte/icons/arrow-left-right';

  import Button from '$lib/components/Button.svelte';
  import DocNewButton from '$lib/components/docs/DocNewButton.svelte';
  import { ICON } from '$lib/components/icon';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { t } from '$lib/i18n/i18n.svelte';

  import { viewMeta } from '../shell/views';
  import DeleteAllMenu from './DeleteAllMenu.svelte';

  interface Props {
    /** The view this is the header of: it names it, and says what "Delete all" deletes. */
    view: 'notes' | 'scripts';
    /** The id of the place in the header that the list on screen puts its search and its filters into. */
    slot: string;
    /** What the add button says. */
    addLabel: string;
    /** There is a Harness Memory to save to: without one the add button is not offered. */
    available: boolean;
    onadd: () => void;
    /** Opens the Sync dialog. Only given while the entries are read from the remote database and this computer together and a copy is turned on, so the button follows. */
    onsync?: () => void;
    /** More items of the "⋯" menu, above "Delete all". They get the menu's `close`. */
    extra?: Snippet<[{ close: () => void }]>;
  }

  let { view, slot, addLabel, available, onadd, onsync, extra }: Props = $props();

  const meta = $derived(viewMeta(view));
</script>

<ViewHeader title={meta.label} icon={meta.icon}>
  {#snippet actions()}
    <div id={slot} class="contents"></div>

    <!-- After the search and the filters: Sync next to the main action. It is a button, not a banner, because entries saved on this computer are normal here. -->
    {#if onsync}
      <Button onclick={onsync}>
        {#snippet leading()}<ArrowLeftRight size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('memory.sync.button')}
      </Button>
    {/if}

    {#if available}
      <DocNewButton label={addLabel} onclick={onadd} />
    {/if}
  {/snippet}

  {#snippet fixed()}
    <DeleteAllMenu section={view} {extra} />
  {/snippet}
</ViewHeader>
