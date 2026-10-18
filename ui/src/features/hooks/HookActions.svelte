<script lang="ts">
  import Copy from '@lucide/svelte/icons/copy';
  import Pencil from '@lucide/svelte/icons/pencil';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import Share2 from '@lucide/svelte/icons/share-2';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import UserMinus from '@lucide/svelte/icons/user-minus';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { HookView } from '$lib/ipc/bindings/HookView';

  import { COPY_BLOCK_TEXT, type CopyBlocked, SHARE_BLOCK_TEXT } from './hook-team';

  interface Props {
    view: HookView;
    /** Why Edit is off (a write would approve what it writes), or null. */
    lockReason: string | null;
    /** Hooks are shared with the team, so a local hook can be shared. */
    sharing: boolean;
    /** The local original of a team hook is there to update the team copy from. */
    canUpdate: boolean;
    /** A change to this hook is being written. */
    busy: boolean;
    onedit: () => void;
    ondelete: () => void;
    onshare: () => void;
    onupdate: () => void;
    onremove: () => void;
    oncopy: () => void;
  }

  let { view, lockReason, sharing, canUpdate, busy, onedit, ondelete, onshare, onupdate, onremove, oncopy }: Props = $props();

  const blocked = $derived(view.shareBlock === null ? null : t(SHARE_BLOCK_TEXT[view.shareBlock]));

  // A team hook is copied only once it can run: one that waits for approval, cannot run or is overridden is never offered.
  const copyBlock = $derived.by(() => {
    const key = COPY_BLOCK_TEXT[view.status as CopyBlocked];

    return key === undefined ? null : t(key);
  });
</script>

<div class="flex flex-wrap items-center gap-2">
  {#if view.origin === 'team'}
    <!-- A team hook is the team's: it is copied, updated from its original or removed, and never edited or deleted here. -->
    <Tooltip text={copyBlock ?? ''} disabled={copyBlock === null}>
      {#snippet children({ describedby })}
        <Button disabled={busy || copyBlock !== null} aria-describedby={describedby} onclick={oncopy}>
          {#snippet leading()}<Copy size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('hooks.team.copy')}
        </Button>
      {/snippet}
    </Tooltip>

    {#if view.teamChanged && canUpdate}
      <Button disabled={busy} onclick={onupdate}>
        {#snippet leading()}<RefreshCw size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('hooks.team.update')}
      </Button>
    {/if}

    <Button variant="dangerQuiet" disabled={busy} onclick={onremove}>
      {#snippet leading()}<UserMinus size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('hooks.team.remove')}
    </Button>
  {:else}
    <!-- A save approves the bytes it writes, so a hook that waits for approval cannot be edited: the user approves or rejects it first. -->
    <Tooltip text={lockReason ?? ''} disabled={lockReason === null}>
      {#snippet children({ describedby })}
        <Button disabled={lockReason !== null} aria-describedby={describedby} onclick={onedit}>
          {#snippet leading()}<Pencil size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('hooks.panel.edit')}
        </Button>
      {/snippet}
    </Tooltip>

    {#if sharing}
      <Tooltip text={blocked ?? ''} disabled={blocked === null}>
        {#snippet children({ describedby })}
          <Button disabled={blocked !== null || busy} aria-describedby={describedby} onclick={onshare}>
            {#snippet leading()}<Share2 size={ICON.sm} aria-hidden="true" />{/snippet}
            {t('hooks.share.action')}
          </Button>
        {/snippet}
      </Tooltip>
    {/if}

    <Button variant="dangerQuiet" onclick={ondelete}>
      {#snippet leading()}<Trash2 size={ICON.sm} aria-hidden="true" />{/snippet}
      {t('hooks.panel.delete')}
    </Button>
  {/if}
</div>
