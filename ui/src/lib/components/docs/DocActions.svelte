<script lang="ts">
  import Copy from '@lucide/svelte/icons/copy';
  import Pencil from '@lucide/svelte/icons/pencil';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import Share2 from '@lucide/svelte/icons/share-2';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import UserMinus from '@lucide/svelte/icons/user-minus';
  import type { Snippet } from 'svelte';

  import Button from '../Button.svelte';
  import { type DocActionHandlers } from './doc-groups';
  import { ICON } from '../icon';
  import Tooltip from '../Tooltip.svelte';

  /** The words of the buttons. */
  export interface DocActionLabels {
    edit: string;
    share: string;
    delete: string;
    copy: string;
    update: string;
    remove: string;
  }

  interface Props extends DocActionHandlers {
    labels: DocActionLabels;
    /** The document is the team's: it is copied, updated from its original or removed, and never edited or deleted here. */
    team: boolean;
    /** Why Edit is off (a write would approve what it writes), or null. */
    lockReason: string | null;
    /** Documents are shared with the team, so a local document can be shared. */
    sharing: boolean;
    /** Why a local document cannot be shared, or null when it can. */
    shareBlock: string | null;
    /** Why a team document cannot be copied, or null when it can. */
    copyBlock: string | null;
    /** The local original of a team document is there, and changed since it was shared, so the team copy can be updated from it. */
    canUpdate: boolean;
    /** A change to this document is being written. */
    busy: boolean;
    /** Buttons that come first, e.g. "Run now". */
    leading?: Snippet;
  }

  let { labels, team, lockReason, sharing, shareBlock, copyBlock, canUpdate, busy, leading, onedit, ondelete, onshare, onupdate, onremove, oncopy }: Props = $props();
</script>

<div class="flex flex-wrap items-center gap-2">
  {@render leading?.()}

  {#if team}
    <Tooltip text={copyBlock ?? ''} disabled={copyBlock === null}>
      {#snippet children({ describedby })}
        <Button disabled={busy || copyBlock !== null} aria-describedby={describedby} onclick={oncopy}>
          {#snippet leading()}<Copy size={ICON.sm} aria-hidden="true" />{/snippet}
          {labels.copy}
        </Button>
      {/snippet}
    </Tooltip>

    {#if canUpdate}
      <Button disabled={busy} onclick={onupdate}>
        {#snippet leading()}<RefreshCw size={ICON.sm} aria-hidden="true" />{/snippet}
        {labels.update}
      </Button>
    {/if}

    <Button variant="dangerQuiet" disabled={busy} onclick={onremove}>
      {#snippet leading()}<UserMinus size={ICON.sm} aria-hidden="true" />{/snippet}
      {labels.remove}
    </Button>
  {:else}
    <!-- A save approves the bytes it writes, so a document that waits for approval cannot be edited: the user approves or rejects it first. -->
    <Tooltip text={lockReason ?? ''} disabled={lockReason === null}>
      {#snippet children({ describedby })}
        <Button disabled={lockReason !== null} aria-describedby={describedby} onclick={onedit}>
          {#snippet leading()}<Pencil size={ICON.sm} aria-hidden="true" />{/snippet}
          {labels.edit}
        </Button>
      {/snippet}
    </Tooltip>

    {#if sharing}
      <Tooltip text={shareBlock ?? ''} disabled={shareBlock === null}>
        {#snippet children({ describedby })}
          <Button disabled={shareBlock !== null || busy} aria-describedby={describedby} onclick={onshare}>
            {#snippet leading()}<Share2 size={ICON.sm} aria-hidden="true" />{/snippet}
            {labels.share}
          </Button>
        {/snippet}
      </Tooltip>
    {/if}

    <Button variant="dangerQuiet" onclick={ondelete}>
      {#snippet leading()}<Trash2 size={ICON.sm} aria-hidden="true" />{/snippet}
      {labels.delete}
    </Button>
  {/if}
</div>
