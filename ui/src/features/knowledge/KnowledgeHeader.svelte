<script lang="ts">
  import DocNewButton from '$lib/components/docs/DocNewButton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import { t, tn } from '$lib/i18n/i18n.svelte';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeImport } from '$lib/stores/knowledgeImport.svelte';

  import { viewMeta } from '../shell/views';
  import ImportChip from './ImportChip.svelte';
  import KnowledgeMenu from './KnowledgeMenu.svelte';

  interface Props {
    /** The id of the place in the header that the list on screen puts its search into. */
    slot: string;
    /** There is a Harness Knowledge to add documents to: without one the add button and the menu are not offered. */
    available: boolean;
  }

  let { slot, available }: Props = $props();

  const meta = viewMeta('knowledge');
  const documents = $derived(knowledgeChanges.pulse?.documents);
</script>

<ViewHeader title={meta.label} icon={meta.icon} context={available && documents !== undefined ? tn('knowledge.count.documents', documents) : undefined}>
  {#snippet actions()}
    {#if available}
      <ImportChip />
    {/if}

    <div id={slot} class="contents"></div>

    <!-- After the search. -->
    {#if available}
      <DocNewButton label={t('knowledge.add')} onclick={() => knowledgeImport.begin()} />
    {/if}
  {/snippet}

  {#snippet fixed()}
    {#if available}
      <KnowledgeMenu />
    {/if}
  {/snippet}
</ViewHeader>
