<script lang="ts">
  import type { ComponentProps } from 'svelte';

  import { reasonArg } from '$lib/chat/harness-tools';
  import { type FoundDoc, ratedDocs } from '$lib/chat/knowledge-tools';
  import { t, tn } from '$lib/i18n/i18n.svelte';

  import FamilyCallRow from './FamilyCallRow.svelte';
  import FamilyFeedbackCard from './FamilyFeedbackCard.svelte';
  import KnowledgeFoundRow from './KnowledgeFoundRow.svelte';

  /** The call and whether the agent still works, and the rated documents the same reply found or opened before (nothing is known of the others but their ids). */
  type Props = Pick<ComponentProps<typeof FamilyCallRow>, 'call' | 'active'> & { found?: ReadonlyMap<string, FoundDoc> };

  let { call, active, found }: Props = $props();

  /** The documents the reply confirms it marked, else the ones the call asked for. */
  const rows = $derived<FoundDoc[]>(ratedDocs(call.args, call.resultText).map((id) => found?.get(id) ?? { id, title: '', section: null, format: null }));
</script>

<FamilyFeedbackCard
  {call}
  {active}
  heading={t('chat.knowledge.feedback.title')}
  hint={rows[0]?.title || tn('chat.knowledge.feedback.summary', rows.length)}
  {rows}
  reason={reasonArg(call.args)}
  none={t('chat.knowledge.feedback.none')}
>
  {#snippet row(doc)}
    <KnowledgeFoundRow row={doc} use={null} />
  {/snippet}
</FamilyFeedbackCard>
