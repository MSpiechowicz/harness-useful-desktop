<script lang="ts">
  import { lineSign, lineSpoken } from '$lib/git/diff-rows';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { DiffLineKind } from '$lib/ipc/bindings/DiffLineKind';
  import type { KnowledgeQuoteKind } from '$lib/ipc/bindings/KnowledgeQuoteKind';
  import type { KnowledgeQuoteLine } from '$lib/ipc/bindings/KnowledgeQuoteLine';
  import { cx } from '$lib/utils/cx';

  import { DIFF_ROW, DIFF_TEXT } from '$lib/components/diff-tones';
  import DiffViewCode from '$lib/components/DiffViewCode.svelte';

  import HiddenText from '../memory/HiddenText.svelte';
  import { revealHidden } from '../memory/script-format';

  interface Props {
    /** The lines a comment is on, as they were when it was written. */
    lines: readonly KnowledgeQuoteLine[];
  }

  let { lines }: Props = $props();

  /** The Changes view's kinds: a quoted line is shown the way its diff row is. */
  const KINDS: Record<KnowledgeQuoteKind, DiffLineKind> = { added: 'add', removed: 'remove', same: 'context' };
</script>

<!-- The commented lines, read-only, in the diff's own tones. A long quote scrolls inside its block, so the card stays short. -->
<div class="chat-radius overflow-hidden border border-line bg-canvas">
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -- a scrollable region must be reachable by keyboard -->
  <div role="region" tabindex="0" aria-label={t('knowledge.drafts.thread.quote')} class="max-h-30 overflow-y-auto">
    {#each lines as quoted, index (index)}
      {@const kind = KINDS[quoted.kind]}

      <div class={cx('flex items-start', DIFF_ROW[kind])} data-kind={kind}>
        <span class="w-10 shrink-0 px-1.5 text-right font-mono text-[13px] leading-[20px] tabular text-fg-3 select-none" aria-hidden="true">{quoted.line}</span>
        <span class={cx('w-4 shrink-0 text-center font-mono text-[14px] leading-[20px] select-none', DIFF_TEXT[kind])} aria-hidden="true">{lineSign(kind)}</span>
        <span class="sr-only">{lineSpoken(kind)}</span>
        <!-- The text is the team's document: a character that does not show (a bidi override, a zero-width space) is shown as its escape on a red chip, as in a proposal, so the quote cannot read differently from what is stored. -->
        {#if revealHidden(quoted.text).count > 0}
          <span class={cx('min-w-0 flex-1 px-2 font-mono text-[14px] leading-[20px] break-all whitespace-pre-wrap', DIFF_TEXT[kind])}><HiddenText text={quoted.text} /></span>
        {:else}
          <DiffViewCode text={quoted.text} {kind} />
        {/if}
      </div>
    {/each}
  </div>
</div>
