<script lang="ts">
  import BookOpenText from '@lucide/svelte/icons/book-open-text';

  import { docUse, type DocUse } from '$lib/chat/knowledge-use';
  import { type KnowledgeRecallRows, type RecallCardKey, recallToolCall } from '$lib/chat/recall-rows';
  import { t } from '$lib/i18n/i18n.svelte';

  import FamilySearchCard from './FamilySearchCard.svelte';
  import KnowledgeFoundRow from './KnowledgeFoundRow.svelte';
  import { knowledgeCounts } from './recall-counts';

  interface Props extends RecallCardKey {
    /** The documents the app found for the message. */
    recall: KnowledgeRecallRows;
    /** What the reply did with them (nothing is known of their use without it). */
    use?: DocUse;
  }

  let { id, recall, use }: Props = $props();

  const call = $derived(recallToolCall(id, 'knowledge_search', recall.text));
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
