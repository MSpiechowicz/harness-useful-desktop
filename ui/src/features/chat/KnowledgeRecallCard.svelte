<script lang="ts">
  import BookOpenText from '@lucide/svelte/icons/book-open-text';

  import { docUse, type DocUse } from '$lib/chat/knowledge-use';
  import type { KnowledgeRecallRows } from '$lib/chat/recall-rows';
  import { blankToolCall } from '$lib/chat/tools';
  import { t } from '$lib/i18n/i18n.svelte';

  import FamilySearchCard from './FamilySearchCard.svelte';
  import KnowledgeFoundRow from './KnowledgeFoundRow.svelte';
  import { knowledgeCounts } from './recall-counts';

  interface Props {
    /** The id the use index knows the app's search of this message by. */
    id: string;
    /** The documents the app found for the message. */
    recall: KnowledgeRecallRows;
    /** What the reply did with them (nothing is known of their use without it). */
    use?: DocUse;
  }

  let { id, recall, use }: Props = $props();

  // The app's own search looks like one of the agent's: a call that finished, with the block it found as its output.
  const call = $derived({ ...blankToolCall(id, 'knowledge_search'), status: 'ok' as const, resultText: recall.text });
</script>

<FamilySearchCard
  {call}
  active={false}
  icon={BookOpenText}
  heading={t('chat.knowledge.recall.title', { counts: knowledgeCounts(recall) })}
  headingRunning=""
  hint={null}
  rows={recall.docs}
  useOf={(doc) => docUse(use, doc.id)}
  ended={use?.ended === true}
  saysNothing={false}
  empty={t('chat.knowledge.search.empty')}
  how={t('chat.knowledge.search.how')}
>
  {#snippet row(doc, mark, dim)}
    <KnowledgeFoundRow row={doc} use={mark} {dim} />
  {/snippet}
</FamilySearchCard>
