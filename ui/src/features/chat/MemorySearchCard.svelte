<script lang="ts">
  import Brain from '@lucide/svelte/icons/brain';
  import type { ComponentProps } from 'svelte';

  import { type FoundRow, parseSearch, textArg } from '$lib/chat/memory-tools';
  import { noteUse, scriptUse, type SearchUse } from '$lib/chat/memory-use';
  import { primaryArg } from '$lib/chat/tool-meta';
  import { isJsonObject } from '$lib/chat/tools';
  import type { RowUse } from '$lib/chat/use-index';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { JsonValue } from '$lib/ipc/bindings/serde_json/JsonValue';
  import type { NoteKind } from '$lib/ipc/bindings/NoteKind';

  import { KIND_LABELS } from '../memory/note-format';

  import FamilyCallRow from './FamilyCallRow.svelte';
  import FamilySearchCard from './FamilySearchCard.svelte';
  import MemoryFoundRow from './MemoryFoundRow.svelte';

  /** The call and whether the agent still works, as the card row takes them, and what the rest of the reply did with the rows (nothing is known of their use without it). */
  type Props = Pick<ComponentProps<typeof FamilyCallRow>, 'call' | 'active'> & { use?: SearchUse };

  let { call, active, use }: Props = $props();

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

  const found = $derived(parseSearch(call.resultText));
  const rows = $derived<FoundRow[]>([...found.notes, ...found.scripts]);

  function useOf(row: FoundRow): RowUse | null {
    return row.type === 'note' ? noteUse(use, row.id) : scriptUse(use, row);
  }
</script>

<FamilySearchCard
  {call}
  {active}
  icon={Brain}
  heading={t('chat.memory.search.title')}
  headingRunning={t('chat.memory.search.titleRunning')}
  hint={searchHint(call.args)}
  {rows}
  {useOf}
  ended={use?.ended === true}
  saysNothing={/no matching notes/i.test(call.resultText ?? '')}
  empty={t('chat.memory.search.empty')}
  how={t('chat.memory.search.how')}
>
  {#snippet row(row, mark, dim)}
    <MemoryFoundRow {row} use={mark} {dim} />
  {/snippet}
</FamilySearchCard>
