<script lang="ts">
  import BookOpenText from '@lucide/svelte/icons/book-open-text';
  import type { ComponentProps } from 'svelte';

  import { parseDocs, saysNothing } from '$lib/chat/knowledge-tools';
  import { docUse, type DocUse } from '$lib/chat/knowledge-use';
  import { textArg } from '$lib/chat/harness-tools';
  import { t } from '$lib/i18n/i18n.svelte';

  import FamilyCallRow from './FamilyCallRow.svelte';
  import FamilySearchCard from './FamilySearchCard.svelte';
  import KnowledgeFoundRow from './KnowledgeFoundRow.svelte';

  /** The call and whether the agent still works, as the card row takes them, and what the rest of the reply did with the documents (nothing is known of their use without it). */
  type Props = Pick<ComponentProps<typeof FamilyCallRow>, 'call' | 'active'> & { use?: DocUse };

  let { call, active, use }: Props = $props();

  /** The words of the search beside the title; nothing while the arguments have not arrived. */
  const query = $derived(textArg(call.args, 'query'));
</script>

<FamilySearchCard
  {call}
  {active}
  icon={BookOpenText}
  heading={t('chat.knowledge.search.title')}
  headingRunning={t('chat.knowledge.search.titleRunning')}
  hint={query === null ? null : `"${query}"`}
  rows={parseDocs(call.resultText)}
  useOf={(doc) => docUse(use, doc.id)}
  ended={use?.ended === true}
  saysNothing={saysNothing(call.resultText)}
  empty={t('chat.knowledge.search.empty')}
  how={t('chat.knowledge.search.how')}
>
  {#snippet row(doc, mark, dim)}
    <KnowledgeFoundRow row={doc} use={mark} {dim} />
  {/snippet}
</FamilySearchCard>
