<script lang="ts">
  import { rise } from '$lib/design/motion';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { KnowledgeHistoryEvent } from '$lib/ipc/bindings/KnowledgeHistoryEvent';
  import { knowledgeViewer } from '$lib/stores/knowledgeViewer.svelte';
  import type { ViewStatus } from '$lib/stores/view-loader.svelte';
  import { cx } from '$lib/utils/cx';
  import { formatDateTime, relativeTime } from '$lib/utils/format';

  import BodyState from '../memory/BodyState.svelte';
  import { COMMENT_TEXT } from './comment-text';
  import { glyphOf, historyEntries, verbsText } from './history-entries';

  interface Props {
    /** The events, newest first; null until they were read. */
    events: readonly KnowledgeHistoryEvent[] | null;
    status: ViewStatus;
    error: string | null;
    onretry: () => void;
  }

  let { events, status, error, onretry }: Props = $props();

  const entries = $derived(historyEntries(events ?? []));

  /** The person as the line names them: "You" for the user, the name otherwise, "Someone" when none was known. */
  function who(name: string | null): string {
    if (name === null) {
      return t('knowledge.drafts.someone');
    }

    return knowledgeViewer.is(name) === true ? t('knowledge.drafts.you') : name;
  }
</script>

<BodyState loaded={events !== null} {status} {error} failed={t('knowledge.drafts.history.failed')} loading={t('knowledge.drafts.history.loading')} {onretry}>
  <!-- A thin rail, newest first. No avatars: the name is medium, the rest of the sentence quiet, the time at the end with the exact moment in its tooltip. -->
  <ol class="flex flex-col gap-3.5">
    {#each entries as entry, index (entry.key)}
      {@const glyph = glyphOf(entry)}
      <li in:rise={{ y: 4, duration: 200 }} class="relative grid grid-cols-[1.25rem_minmax(0,1fr)] items-start gap-x-2.5">
        <!-- The rail: a hairline from under the icon through the row gap to the next event, so it follows a second line or a quoted comment. The icon's box has the panel's colour and masks it. The last event has none under it. -->
        {#if index < entries.length - 1}
          <span data-history-rail aria-hidden="true" class="absolute top-[1.375rem] -bottom-3.5 left-[calc(0.625rem-0.5px)] w-px bg-line"></span>
        {/if}
        <span class={cx('relative z-10 grid size-5 place-items-center bg-surface-1', glyph.tone ?? 'text-fg-3')}><glyph.icon size={14} aria-hidden="true" /></span>

        <div class="min-w-0">
          <p class="flex min-h-5 items-baseline gap-2 text-sm leading-5 text-fg-2">
            <span class="min-w-0"><span class="font-medium text-fg">{who(entry.by)}</span> {verbsText(entry)}</span>
            <time datetime={entry.at} title={formatDateTime(entry.at)} class="tabular ml-auto shrink-0 text-xs text-fg-3">{relativeTime(entry.at)}</time>
          </p>

          {#if entry.comment}<p class="mt-1 {COMMENT_TEXT}">{entry.comment}</p>{/if}
        </div>
      </li>
    {/each}
  </ol>
</BodyState>
