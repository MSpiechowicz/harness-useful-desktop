<script lang="ts">
  import Brain from '@lucide/svelte/icons/brain';

  import type { FoundRow } from '$lib/chat/memory-tools';
  import { noteUse, scriptUse, type SearchUse } from '$lib/chat/memory-use';
  import { type MemoryRecallRows, type RecallCardKey, recallToolCall } from '$lib/chat/recall-rows';
  import type { RowUse } from '$lib/chat/use-index';
  import { t } from '$lib/i18n/i18n.svelte';

  import FamilySearchCard from './FamilySearchCard.svelte';
  import MemoryFoundRow from './MemoryFoundRow.svelte';
  import { memoryCounts } from './recall-counts';

  interface Props extends RecallCardKey {
    /** The notes and scripts the app found for the message. */
    recall: MemoryRecallRows;
    /** What the reply did with them (nothing is known of their use without it). */
    use?: SearchUse;
  }

  let { id, recall, use }: Props = $props();

  const call = $derived(recallToolCall(id, 'note_search', recall.text));
  const rows = $derived<FoundRow[]>([...recall.notes, ...recall.scripts]);

  function useOf(row: FoundRow): RowUse | null {
    return row.type === 'note' ? noteUse(use, row.id) : scriptUse(use, row);
  }
</script>

<FamilySearchCard
  {call}
  active={false}
  icon={Brain}
  heading={t('chat.memory.recall.title', { counts: memoryCounts(recall) })}
  headingRunning=""
  hint={null}
  {rows}
  {useOf}
  ended={use?.ended === true}
  saysNothing={false}
  empty={t('chat.memory.search.empty')}
  how={t('chat.memory.search.how')}
>
  {#snippet row(row, mark, dim)}
    <MemoryFoundRow {row} use={mark} {dim} />
  {/snippet}
</FamilySearchCard>
