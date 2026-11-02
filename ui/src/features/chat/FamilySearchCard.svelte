<script lang="ts" generics="Row">
  import type { ComponentProps, Snippet } from 'svelte';

  import type { IconComponent } from '$lib/components/icon';
  import { type RowUse, sharedReason } from '$lib/chat/use-index';
  import { t } from '$lib/i18n/i18n.svelte';
  import { formatCount } from '$lib/utils/format';

  import FamilyCallRow from './FamilyCallRow.svelte';
  import ToolResult from './ToolResult.svelte';

  interface Props extends Pick<ComponentProps<typeof FamilyCallRow>, 'call' | 'active'> {
    icon: IconComponent;
    /** The title while the search is done and while it runs. */
    heading: string;
    headingRunning: string;
    /** Beside the title: what the search asked. */
    hint: string | null;
    /** The rows the reply lists, in its order. */
    rows: Row[];
    /** What the reply did with a row; null for none (and for every row until the reply is over). */
    useOf: (row: Row) => RowUse | null;
    /** The reply that made the search is over, so what helped is known. Until then the agent may still say it, so nothing is marked and nothing is counted. */
    ended: boolean;
    /** The reply says plainly that nothing matched, so an empty list is a finding and not a format this app does not read. */
    saysNothing: boolean;
    /** What the body says for a search that found nothing, and what the footer says about how rows are marked. */
    empty: string;
    how: string;
    /** One row, with what the reply did with it and whether it reads quieter. */
    row: Snippet<[Row, RowUse | null, boolean]>;
  }

  let { call, active, icon, heading, headingRunning, hint, rows, useOf, ended, saysNothing, empty, how, row: rowOf }: Props = $props();

  let raw = $state(false);

  const searching = $derived(call.status === 'running' || (call.status === 'pending' && active));
  const judged = $derived(call.status === 'ok' && ended);

  const marked = $derived(rows.map((row) => ({ row, mark: judged ? useOf(row) : null })));
  const used = $derived(marked.filter((entry) => entry.mark !== null));
  /** One reason for all the useful rows is said once under their caption, not under every row. */
  const reasonOfAll = $derived(sharedReason(used.map((entry) => entry.mark)));
  const unused = $derived(marked.filter((entry) => entry.mark === null));

  /** A finished reply with text that is neither a row nor the plain "nothing matched": a format this app does not read, so it is shown as it is and nothing is said about the count. */
  const unreadable = $derived(call.status === 'ok' && rows.length === 0 && (call.resultText ?? '').trim() !== '' && !saysNothing);

  const done = $derived.by((): { tone: 'info' | 'idle'; label: string } | null => {
    if (unreadable) {
      return null;
    }
    if (rows.length === 0) {
      return { tone: 'idle', label: t('chat.family.search.nothing') };
    }
    if (!judged) {
      return { tone: 'info', label: t('chat.family.search.found', { count: formatCount(rows.length) }) };
    }

    return { tone: used.length > 0 ? 'info' : 'idle', label: t('chat.family.search.useful', { used: formatCount(used.length) }) };
  });
</script>

{#snippet captioned(caption: string, list: typeof marked, dim: boolean, shared: string | null = null)}
  <section class="border-t border-line first:border-t-0">
    <p class="px-[18px] pt-3 pb-1.5 text-xs text-fg-3">{caption}</p>
    {#if shared}<p class="line-clamp-2 px-[18px] pb-2 text-[12.5px] leading-snug text-fg-2" title={shared}>{t('chat.family.row.reason', { reason: shared })}</p>{/if}
    <ul class="flex flex-col divide-y divide-line">
      {#each list as { row, mark }, index (index)}
        {@render rowOf(row, shared && mark?.kind === 'useful' ? { kind: 'useful', reason: null } : mark, dim)}
      {/each}
    </ul>
  </section>
{/snippet}

<FamilyCallRow {call} {active} {icon} heading={searching ? headingRunning : heading} {hint} {done} runningLabel={t('chat.family.search.running')}>
  {#snippet content()}
    {#if unreadable}
      <div class="px-4 py-4">
        <ToolResult text={call.resultText} running={false} failed={false} />
      </div>
    {:else if rows.length === 0}
      <p class="px-[18px] py-3.5 text-sm text-fg-3">{empty}</p>
    {:else}
      {#if !judged}
        {@render captioned(t('chat.family.search.caption.found'), unused, false)}
      {:else}
        {#if used.length > 0}
          {@render captioned(t('chat.family.search.caption.useful'), used, false, reasonOfAll)}
        {/if}
        {#if unused.length > 0}
          {@render captioned(used.length > 0 ? t('chat.family.search.caption.unused') : t('chat.family.search.caption.noneUsed'), unused, true)}
        {/if}
      {/if}

      <div class="flex flex-col gap-2 border-t border-line px-[18px] py-3">
        <div class="flex items-center justify-between gap-3 text-xs text-fg-3">
          {#if judged}<span>{how}</span>{/if}
          <button
            type="button"
            aria-expanded={raw}
            class="ml-auto shrink-0 rounded-sm text-accent-text underline underline-offset-2 transition-colors hover:text-fg"
            onclick={() => (raw = !raw)}
          >
            {raw ? t('chat.family.search.hideRaw') : t('chat.family.search.showRaw')}
          </button>
        </div>
        {#if raw}<ToolResult text={call.resultText} running={false} failed={false} />{/if}
      </div>
    {/if}
  {/snippet}
</FamilyCallRow>
