<script lang="ts">
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import type { Snippet } from 'svelte';

  import { cx } from '$lib/utils/cx';

  import Button from '../Button.svelte';
  import { DIFF_ROW, DIFF_TEXT } from '../diff-tones';
  import { type DocReviewHandlers, type ReviewChange } from './doc-diff';
  import { ICON } from '../icon';
  import { NOTICE_LOOKS } from '../notice';
  import NoticeHead from '../NoticeHead.svelte';

  interface Props extends DocReviewHandlers {
    /** What happened to the document: "An agent added this hook". */
    title: string;
    body: string;
    changes: ReviewChange[];
    /** Names a diff for assistive technology: "Changes to Name". */
    diffLabel: (label: string) => string;
    /** Draws the line diff of a text part. */
    textDiff: Snippet<[ReviewChange]>;
    /** More under the changes, e.g. that the document runs tools without asking. */
    extra?: Snippet;
    approveLabel: string;
    /** Left out when the user can only approve or leave the document. */
    rejectLabel?: string;
  }

  let { title, body, changes, diffLabel, textDiff, extra, approveLabel, rejectLabel, busy, onapprove, onreject }: Props = $props();

  const look = NOTICE_LOOKS.warning;

  // A part with nothing on either side has no row to show, and without any part there is no box of changes: an empty dark
  // box inside the notice reads as something that failed to load.
  const shown = $derived(changes.filter((change) => Boolean(change.from || change.to)));
</script>

<!-- Nothing an agent wrote runs before this: the user reads exactly what is new, and approves that version or sends it away. -->
<div class={cx('flex flex-col gap-3 rounded-md p-3', look.box)} role="status">
  <NoticeHead level="warning" {title} {body} />

  {#if shown.length > 0}
    <div class="flex flex-col gap-3 rounded-md border border-line bg-surface-1 p-3" data-review-changes>
      {#each shown as change (change.key)}
        <div class="flex flex-col gap-1.5">
          <p class="text-xs font-medium tracking-wider text-fg-3 uppercase">{change.label}</p>

          {#if change.text}
            {@render textDiff(change)}
          {:else}
            <div class="chat-radius overflow-hidden border border-line bg-canvas py-1 text-sm leading-5 break-words" role="group" aria-label={diffLabel(change.label)}>
              {#if change.from !== null}
                <p class={cx('grid grid-cols-[1.5ch_1fr] gap-x-2 px-3.5', DIFF_ROW.remove, DIFF_TEXT.remove, change.mono && 'font-mono')}><span aria-hidden="true">−</span><span>{change.from}</span></p>
              {/if}
              {#if change.to !== null}
                <p class={cx('grid grid-cols-[1.5ch_1fr] gap-x-2 px-3.5', DIFF_ROW.add, DIFF_TEXT.add, change.mono && 'font-mono')}><span aria-hidden="true">+</span><span>{change.to}</span></p>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  {@render extra?.()}

  <div class="flex flex-wrap items-center gap-2">
    <Button variant="primary" disabled={busy} onclick={onapprove}>
      {#snippet leading()}<ShieldCheck size={ICON.sm} aria-hidden="true" />{/snippet}
      {approveLabel}
    </Button>
    {#if onreject && rejectLabel}
      <Button variant="dangerQuiet" disabled={busy} onclick={onreject}>{rejectLabel}</Button>
    {/if}
  </div>
</div>
