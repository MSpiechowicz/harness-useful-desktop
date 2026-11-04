<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import Lock from '@lucide/svelte/icons/lock';
  import MessageSquareWarning from '@lucide/svelte/icons/message-square-warning';
  import X from '@lucide/svelte/icons/x';

  import Button from '$lib/components/Button.svelte';
  import { ICON } from '$lib/components/icon';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeHeldItem } from '$lib/ipc/bindings/KnowledgeHeldItem';
  import { uid } from '$lib/utils/ids';

  import GrowField from './GrowField.svelte';

  interface Props {
    item: KnowledgeHeldItem;
    /** The whole text was read: only then can the proposal be approved. */
    approvable: boolean;
    /** The proposal is being decided. */
    busy: boolean;
    /** The chat of the agent that proposed it is open in this app, so a comment can reach it. */
    followUp: boolean;
    onapprove: () => void;
    /** Asks the agent for changes with a comment. */
    onrequest: (comment: string) => void;
    /** Throws the proposal away, with a reason for the agent when one was written. */
    onreject: (reason: string | null) => void;
  }

  let { item, approvable, busy, followUp, onapprove, onrequest, onreject }: Props = $props();

  type Mode = 'row' | 'request' | 'reject';

  const reasonId = uid('held-reason');
  let mode = $state<Mode>('row');
  let text = $state('');

  // Another proposal starts with nothing half written.
  $effect(() => {
    void item.id;

    mode = 'row';
    text = '';
  });

  const asked = $derived(item.status === 'changesRequested');
  // An agent can only be told when its chat is open here: without that the request would reach nobody.
  const requestReason = $derived(followUp ? null : t('knowledge.drafts.held.noFollowUp'));

  function open(next: Mode): void {
    mode = next;
    text = '';
  }
</script>

<div class="flex flex-col gap-3">
  {#if mode === 'request'}
    <div in:rise={{ y: 4, duration: 200 }} class="flex flex-col gap-3">
      <GrowField bind:value={text} label={t('knowledge.drafts.held.requestLabel')} note={t('knowledge.drafts.required')} placeholder={t('knowledge.drafts.held.requestPlaceholder')} maxlength={500} onsubmit={() => text.trim() !== '' && onrequest(text.trim())} oncancel={() => (mode = 'row')} />

      <div class="flex flex-nowrap items-center gap-2">
        <Button variant="primary" loading={busy} disabled={text.trim() === ''} onclick={() => onrequest(text.trim())}>
          {#snippet leading()}<MessageSquareWarning size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('knowledge.drafts.review.requestChanges')}
        </Button>

        <Button onclick={() => (mode = 'row')}>{t('common.cancel')}</Button>
      </div>
    </div>
  {:else if mode === 'reject'}
    <div in:rise={{ y: 4, duration: 200 }} class="flex flex-col gap-3">
      <GrowField bind:value={text} label={t('knowledge.drafts.held.rejectLabel')} note={t('knowledge.drafts.optional')} placeholder={t('knowledge.drafts.held.rejectPlaceholder')} maxlength={500} oncancel={() => (mode = 'row')} />

      <div class="flex flex-nowrap items-center gap-2">
        <Button variant="dangerQuiet" disabled={busy} onclick={() => onreject(text.trim() === '' ? null : text.trim())}>
          {#snippet leading()}<X size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('knowledge.held.reject')}
        </Button>

        <Button onclick={() => (mode = 'row')}>{t('common.cancel')}</Button>
      </div>
    </div>
  {:else}
    <!-- One row, left aligned, never wrapping: approve, then ask for changes, then the quiet destructive one. -->
    <div class="flex flex-nowrap items-center gap-2">
      <Button variant="primary" loading={busy} disabled={!approvable} onclick={onapprove}>
        {#snippet leading()}<Check size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('knowledge.held.approve')}
      </Button>

      {#if !asked}
        <Tooltip text={requestReason ?? ''} disabled={requestReason === null} side="top">
          {#snippet children({ describedby })}
            <span aria-describedby={requestReason ? describedby : undefined} class="inline-flex">
              <Button disabled={busy || requestReason !== null} aria-describedby={requestReason ? reasonId : undefined} onclick={() => open('request')}>
                {#snippet leading()}<MessageSquareWarning size={ICON.sm} aria-hidden="true" />{/snippet}
                {t('knowledge.drafts.review.requestChanges')}
              </Button>
            </span>
          {/snippet}
        </Tooltip>
      {/if}

      <Button variant="dangerQuiet" disabled={busy} onclick={() => open('reject')}>
        {#snippet leading()}<X size={ICON.sm} aria-hidden="true" />{/snippet}
        {t('knowledge.held.reject')}
      </Button>
    </div>

    {#if !asked && requestReason}
      <p id={reasonId} class="flex items-start gap-2 text-sm text-fg-3"><Lock size={ICON.sm} class="mt-0.5 shrink-0" aria-hidden="true" />{requestReason}</p>
    {/if}
  {/if}
</div>
