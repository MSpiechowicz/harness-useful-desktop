<script lang="ts">
  import Brain from '@lucide/svelte/icons/brain';

  import type { FoundRow } from '$lib/chat/memory-tools';
  import { noteUse, scriptUse, type SearchUse } from '$lib/chat/memory-use';
  import type { MemoryRecallRows } from '$lib/chat/recall-rows';
  import { blankToolCall } from '$lib/chat/tools';
  import type { RowUse } from '$lib/chat/use-index';
  import { t } from '$lib/i18n/i18n.svelte';

  import FamilySearchCard from './FamilySearchCard.svelte';
  import MemoryFoundRow from './MemoryFoundRow.svelte';
  import { memoryCounts } from './recall-counts';

  interface Props {
    /** The id the use index knows the app's search of this message by. */
    id: string;
    /** The notes and scripts the app found for the message. */
    recall: MemoryRecallRows;
    /** What the reply did with them (nothing is known of their use without it). */
    use?: SearchUse;
  }

  let { id, recall, use }: Props = $props();

  // The app's own search looks like one of the agent's: a call that finished, with the block it found as its output.
  const call = $derived({ ...blankToolCall(id, 'note_search'), status: 'ok' as const, resultText: recall.text });
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
