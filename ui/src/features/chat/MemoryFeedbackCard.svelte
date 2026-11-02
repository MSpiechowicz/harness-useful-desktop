<script lang="ts">
  import type { ComponentProps } from 'svelte';

  import { reasonArg } from '$lib/chat/harness-tools';
  import { type FoundNote, ratedNotes, type UnknownNote } from '$lib/chat/memory-tools';
  import { t, tn } from '$lib/i18n/i18n.svelte';

  import FamilyCallRow from './FamilyCallRow.svelte';
  import FamilyFeedbackCard from './FamilyFeedbackCard.svelte';
  import MemoryFoundRow from './MemoryFoundRow.svelte';

  /** The call and whether the agent still works, and the rated notes the same reply found or read before (nothing is known of the others but their ids). */
  type Props = Pick<ComponentProps<typeof FamilyCallRow>, 'call' | 'active'> & { found?: ReadonlyMap<number, FoundNote> };

  let { call, active, found }: Props = $props();

  /** The notes the reply confirms it marked, else the ones the call asked for. */
  const rows = $derived<(FoundNote | UnknownNote)[]>(ratedNotes(call.args, call.resultText).map((id) => found?.get(id) ?? { type: 'unknown', id }));

  /** The first note's title beside the heading, as the read card does; the count says it while no title is known. */
  const first = $derived(rows[0]?.type === 'note' ? rows[0].title : null);
</script>

<FamilyFeedbackCard
  {call}
  {active}
  heading={t('chat.memory.feedback.title')}
  hint={first ?? tn('chat.memory.feedback.summary', rows.length)}
  {rows}
  reason={reasonArg(call.args)}
  none={t('chat.memory.feedback.none')}
>
  {#snippet row(row)}
    <MemoryFoundRow {row} use={null} />
  {/snippet}
</FamilyFeedbackCard>
