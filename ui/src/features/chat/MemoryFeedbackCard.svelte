<script lang="ts">
  import ThumbsUp from '@lucide/svelte/icons/thumbs-up';
  import type { ComponentProps } from 'svelte';

  import { type FoundNote, ratedNotes, textArg, type UnknownNote } from '$lib/chat/memory-tools';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { formatCount } from '$lib/utils/format';

  import MemoryCallRow from './MemoryCallRow.svelte';
  import MemoryFoundRow from './MemoryFoundRow.svelte';

  /** The call and whether the agent still works, and the rated notes the same reply found or read before (nothing is known of the others but their ids). */
  type Props = Pick<ComponentProps<typeof MemoryCallRow>, 'call' | 'active'> & { found?: ReadonlyMap<number, FoundNote> };

  let { call, active, found }: Props = $props();

  /** The notes the reply confirms it marked, else the ones the call asked for. */
  const rows = $derived<(FoundNote | UnknownNote)[]>(ratedNotes(call.args, call.resultText).map((id) => found?.get(id) ?? { type: 'unknown', id }));
  const count = $derived(rows.length);
  const reason = $derived(textArg(call.args, 'reason'));

  /** The first note's title beside the heading, as the read card does; the count says it while no title is known. */
  const first = $derived(rows[0]?.type === 'note' ? rows[0].title : null);
  const hint = $derived(first ?? tn('chat.memory.feedback.summary', count));
</script>

<MemoryCallRow
  {call}
  {active}
  icon={ThumbsUp}
  heading={t('chat.memory.feedback.title')}
  hint={call.status === 'error' ? null : hint}
  done={{ tone: count > 0 ? 'info' : 'idle', label: t('chat.memory.feedback.pill', { count: formatCount(count) }) }}
>
  {#snippet content()}
    {#if count === 0}
      <p class="px-[18px] py-3.5 text-sm text-fg-3">{t('chat.memory.feedback.none')}</p>
    {:else}
      <section>
        <p class="px-[18px] pt-3 pb-1.5 text-xs text-fg-3">{t('chat.memory.search.caption.useful')}</p>
        {#if reason}<p class="px-[18px] pb-2 text-[12.5px] leading-snug text-fg-2">{t('chat.memory.row.reason', { reason })}</p>{/if}
        <ul class="flex flex-col divide-y divide-line">
          {#each rows as row (row.id)}
            <MemoryFoundRow {row} use={null} />
          {/each}
        </ul>
      </section>
    {/if}
  {/snippet}
</MemoryCallRow>
