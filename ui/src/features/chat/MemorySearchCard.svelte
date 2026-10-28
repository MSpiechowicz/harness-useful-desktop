<script lang="ts">
  import BookOpen from '@lucide/svelte/icons/book-open';
  import type { ComponentProps } from 'svelte';

  import { type FoundRow, parseSearch, textArg } from '$lib/chat/memory-tools';
  import { noteUse, type RowUse, scriptUse, type SearchUse, sharedReason } from '$lib/chat/memory-use';
  import { primaryArg } from '$lib/chat/tool-meta';
  import { isJsonObject } from '$lib/chat/tools';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';
  import type { NoteKind } from '$lib/ipc/bindings/NoteKind';
  import { formatCount } from '$lib/utils/format';

  import { KIND_LABELS } from '../memory/note-format';

  import MemoryCallRow from './MemoryCallRow.svelte';
  import MemoryFoundRow from './MemoryFoundRow.svelte';
  import ToolResult from './ToolResult.svelte';

  /** The call and whether the agent still works, as the card row takes them, and what the rest of the reply did with the rows (nothing is known of their use without it). */
  type Props = Pick<ComponentProps<typeof MemoryCallRow>, 'call' | 'active'> & { use?: SearchUse };

  let { call, active, use }: Props = $props();

  let raw = $state(false);

  /**
   * What the search asked, beside the title: the words, else the kind it listed, else (an empty or missing query) that it listed
   * the most recent notes. Nothing while the arguments have not arrived.
   */
  function searchHint(args: JsonValue | null): string | null {
    if (!isJsonObject(args)) {
      return null;
    }

    const query = primaryArg(args);
    if (query) {
      return `"${query}"`;
    }

    const kind = textArg(args, 'kind');
    if (kind) {
      return t('chat.memory.search.hint.kind', { kind: kind in KIND_LABELS ? t(KIND_LABELS[kind as NoteKind]) : kind });
    }

    return t('chat.memory.search.hint.recent');
  }

  const hint = $derived(searchHint(call.args));
  const searching = $derived(call.status === 'running' || (call.status === 'pending' && active));

  const found = $derived(parseSearch(call.resultText));
  const rows = $derived<FoundRow[]>([...found.notes, ...found.scripts]);
  /** Until the reply ends the agent may still say what helped, so nothing is marked and nothing is counted. */
  const judged = $derived(call.status === 'ok' && use?.ended === true);

  function useOf(row: FoundRow): RowUse | null {
    return row.type === 'note' ? noteUse(use, row.id) : scriptUse(use, row);
  }

  const marked = $derived(rows.map((row) => ({ row, mark: judged ? useOf(row) : null })));
  const used = $derived(marked.filter((entry) => entry.mark !== null));
  /** One reason for all the useful rows is said once under their caption, not under every row. */
  const reasonOfAll = $derived(sharedReason(used.map((entry) => entry.mark)));
  const unused = $derived(marked.filter((entry) => entry.mark === null));

  /** A finished reply with text that is neither a row nor the plain "no matching notes": a format this app does not read, so it is shown as it is and nothing is said about the count. */
  const unreadable = $derived(call.status === 'ok' && rows.length === 0 && (call.resultText ?? '').trim() !== '' && !/no matching notes/i.test(call.resultText ?? ''));

  const done = $derived.by((): { tone: 'info' | 'idle'; label: string } | null => {
    if (unreadable) {
      return null;
    }
    if (rows.length === 0) {
      return { tone: 'idle', label: t('chat.memory.search.nothing') };
    }
    if (!judged) {
      return { tone: 'info', label: t('chat.memory.search.found', { count: formatCount(rows.length) }) };
    }

    return { tone: used.length > 0 ? 'info' : 'idle', label: t('chat.memory.search.useful', { used: formatCount(used.length) }) };
  });
</script>

{#snippet captioned(caption: string, list: typeof marked, dim: boolean, shared: string | null = null)}
  <section class="border-t border-line first:border-t-0">
    <p class="px-[18px] pt-3 pb-1.5 text-xs text-fg-3">{caption}</p>
    {#if shared}<p class="px-[18px] pb-2 text-[12.5px] leading-snug text-fg-2">{t('chat.memory.row.reason', { reason: shared })}</p>{/if}
    <ul class="flex flex-col divide-y divide-line">
      {#each list as { row, mark }, index (index)}
        <MemoryFoundRow {row} use={shared && mark?.kind === 'useful' ? { kind: 'useful', reason: null } : mark} {dim} />
      {/each}
    </ul>
  </section>
{/snippet}

<MemoryCallRow {call} {active} icon={BookOpen} heading={searching ? t('chat.memory.search.titleRunning') : t('chat.memory.search.title')} {hint} {done} runningLabel={t('chat.memory.search.running')}>
  {#snippet content()}
    {#if unreadable}
      <div class="px-4 py-4">
        <ToolResult text={call.resultText} running={false} failed={false} />
      </div>
    {:else if rows.length === 0}
      <p class="px-[18px] py-3.5 text-sm text-fg-3">{t('chat.memory.search.empty')}</p>
    {:else}
      {#if !judged}
        {@render captioned(t('chat.memory.search.caption.found'), unused, false)}
      {:else}
        {#if used.length > 0}
          {@render captioned(t('chat.memory.search.caption.useful'), used, false, reasonOfAll)}
        {/if}
        {#if unused.length > 0}
          {@render captioned(used.length > 0 ? t('chat.memory.search.caption.unused') : t('chat.memory.search.caption.noneUsed'), unused, true)}
        {/if}
      {/if}

      <div class="flex flex-col gap-2 border-t border-line px-[18px] py-3">
        <div class="flex items-center justify-between gap-3 text-xs text-fg-3">
          {#if judged}<span>{t('chat.memory.search.how')}</span>{/if}
          <button
            type="button"
            aria-expanded={raw}
            class="ml-auto shrink-0 rounded-sm text-accent-text underline underline-offset-2 transition-colors hover:text-fg"
            onclick={() => (raw = !raw)}
          >
            {raw ? t('chat.memory.search.hideRaw') : t('chat.memory.search.showRaw')}
          </button>
        </div>
        {#if raw}<ToolResult text={call.resultText} running={false} failed={false} />{/if}
      </div>
    {/if}
  {/snippet}
</MemoryCallRow>
