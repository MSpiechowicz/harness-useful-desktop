<script lang="ts">
  import Check from '@lucide/svelte/icons/check';
  import CircleCheck from '@lucide/svelte/icons/circle-check';
  import Lock from '@lucide/svelte/icons/lock';
  import MessageSquareWarning from '@lucide/svelte/icons/message-square-warning';
  import Replace from '@lucide/svelte/icons/replace';
  import Send from '@lucide/svelte/icons/send';
  import Undo2 from '@lucide/svelte/icons/undo-2';

  import Button from '$lib/components/Button.svelte';
  import { ICON, type IconComponent } from '$lib/components/icon';
  import Tooltip from '$lib/components/Tooltip.svelte';
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import { formatDateTime, relativeTime } from '$lib/utils/format';
  import { uid } from '$lib/utils/ids';

  import { COMMENT_TEXT } from './comment-text';
  import GrowField from './GrowField.svelte';
  import type { ReviewAction, ReviewModel, ReviewStep } from './review-model';

  interface Props {
    model: ReviewModel;
    /** A step is being made: the buttons wait. */
    busy: boolean;
    /** The user chose a step. `text` is the comment of a request for changes, or the note that goes with a new file. */
    onstep: (step: ReviewStep, text: string) => void;
  }

  let { model, busy, onstep }: Props = $props();

  const ICONS: Record<ReviewStep, IconComponent> = { approve: Check, requestChanges: MessageSquareWarning, ask: Send, askAgain: Send, goLive: CircleCheck, takeBack: Undo2, replace: Replace };
  const reasonId = uid('review-reason');

  /** The reviewer is writing what should change: the row of buttons becomes the field's own row. */
  let composing = $state(false);
  let comment = $state('');
  let note = $state('');

  // A different version (another document, or this one after a step) starts with nothing half written.
  const identity = $derived(`${model.headline}|${model.actions.map((action) => action.step).join()}`);

  $effect(() => {
    void identity;

    composing = false;
    comment = '';
  });

  /** A note goes with a new file, so asking again without one would lose it: that button says so instead of dropping the note. */
  const actions = $derived<ReviewAction[]>(model.actions.map((action) => (action.step === 'askAgain' && action.reason === null && note.trim() !== '' ? { ...action, reason: t('knowledge.drafts.review.noteWithFile') } : action)));
  const reasons = $derived([...new Set(actions.map((action) => action.reason).filter((reason): reason is string => reason !== null))]);
  /** The request for changes being written waits too when the diff on screen is no longer the version's latest text. */
  const composeBlock = $derived(actions.find((action) => action.step === 'requestChanges')?.reason ?? null);

  function press(action: ReviewAction): void {
    if (action.step === 'requestChanges') {
      composing = true;
    } else {
      onstep(action.step, action.step === 'replace' ? note : '');
    }
  }

  function send(): void {
    if (comment.trim() !== '' && composeBlock === null) {
      onstep('requestChanges', comment.trim());
    }
  }
</script>

<div class="flex flex-col gap-3">
  <div>
    <p class="text-base font-medium text-fg">{model.headline}</p>
    {#if model.context}<p class="mt-1 text-sm text-fg-3">{model.context}</p>{/if}
  </div>

  {#if model.quote}
    <blockquote class="flex flex-col gap-1 border-l-2 border-line-strong pl-3">
      <p class="flex items-baseline gap-2 text-xs text-fg-3">
        <span class="min-w-0 truncate">{model.quote.heading}</span>
        {#if model.quote.at}<time datetime={model.quote.at} title={formatDateTime(model.quote.at)} class="ml-auto shrink-0">{relativeTime(model.quote.at)}</time>{/if}
      </p>
      <p class={COMMENT_TEXT}>{model.quote.text}</p>
    </blockquote>
  {/if}

  {#if composing}
    <div in:rise={{ y: 4, duration: 200 }} class="flex flex-col gap-3">
      <GrowField bind:value={comment} label={t('knowledge.drafts.review.commentLabel')} note={t('knowledge.drafts.required')} placeholder={t('knowledge.drafts.review.commentPlaceholder')} maxlength={500} onsubmit={send} oncancel={() => (composing = false)} />

      <div class="flex flex-nowrap items-center gap-2">
        <Button variant="primary" loading={busy} disabled={comment.trim() === '' || composeBlock !== null} onclick={send}>
          {#snippet leading()}<MessageSquareWarning size={ICON.sm} aria-hidden="true" />{/snippet}
          {t('knowledge.drafts.review.requestChanges')}
        </Button>

        <Button onclick={() => (composing = false)}>{t('common.cancel')}</Button>
      </div>

      {#if composeBlock}
        <p in:rise={{ y: 4, duration: 200 }} class="flex items-start gap-2 text-sm text-fg-3"><Lock size={ICON.sm} class="mt-0.5 shrink-0" aria-hidden="true" />{composeBlock}</p>
      {/if}
    </div>
  {:else}
    {#if model.noteField}
      <GrowField bind:value={note} label={t('knowledge.drafts.review.noteLabel')} note={t('knowledge.drafts.optional')} placeholder={t('knowledge.drafts.review.notePlaceholder')} maxlength={500} />
    {/if}

    {#if actions.length > 0}
      <!-- One row, left aligned, never wrapping: the main step first, then the other, then a destructive one. -->
      <div class="flex flex-nowrap items-center gap-2">
        {#each actions as action (action.step)}
          {@const Icon = ICONS[action.step]}
          <Tooltip text={action.reason ?? ''} disabled={action.reason === null} side="top">
            {#snippet children({ describedby })}
              <span aria-describedby={action.reason ? describedby : undefined} class="inline-flex">
                <Button variant={action.variant} disabled={action.reason !== null || busy} loading={busy && action.variant === 'primary'} aria-describedby={action.reason ? reasonId : undefined} onclick={() => press(action)}>
                  {#snippet leading()}<Icon size={ICON.sm} aria-hidden="true" />{/snippet}
                  {action.label}
                </Button>
              </span>
            {/snippet}
          </Tooltip>
        {/each}
      </div>
    {/if}

    {#each reasons as reason, index (reason)}
      <p id={index === 0 ? reasonId : undefined} in:rise={{ y: 4, duration: 200 }} class="flex items-start gap-2 text-sm text-fg-3"><Lock size={ICON.sm} class="mt-0.5 shrink-0" aria-hidden="true" />{reason}</p>
    {/each}
  {/if}
</div>
