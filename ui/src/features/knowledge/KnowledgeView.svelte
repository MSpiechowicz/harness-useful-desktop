<script lang="ts">
  import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
  import { knowledgeHeld } from '$lib/stores/knowledgeHeld.svelte';
  import { knowledgeStatus } from '$lib/stores/knowledgeStatus.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import { provideHeaderActions } from '../memory/HeaderActions.svelte';
  import DropOverlay from './DropOverlay.svelte';
  import ImportDialog from './ImportDialog.svelte';
  import KnowledgeDocs from './KnowledgeDocs.svelte';
  import KnowledgeEmbedNotice from './KnowledgeEmbedNotice.svelte';
  import { useKnowledgeData } from './knowledge-data.svelte';
  import KnowledgeHeader from './KnowledgeHeader.svelte';
  import KnowledgeOffNotice from './KnowledgeOffNotice.svelte';
  import KnowledgeOfflineBanner from './KnowledgeOfflineBanner.svelte';
  import KnowledgeReview from './KnowledgeReview.svelte';
  import KnowledgeUnavailable from './KnowledgeUnavailable.svelte';

  useKnowledgeData();

  const headerActions = provideHeaderActions('knowledge-header-actions');

  // What the list on screen says about Harness Knowledge: missing, too old, lacking something in its database, or not answering.
  const list = $derived(viewState.knowledgeTab === 'review' ? knowledgeHeld.list : knowledgeDocs.list);
  const unavailable = $derived(list.data && list.data.availability.type !== 'available' ? list.data.availability : null);
  const available = $derived(knowledgeDocs.list.data?.availability.type === 'available' && knowledgeDocs.list.status !== 'error');

  /** Reads what the server says again, once Harness Knowledge was installed or updated. */
  async function refresh(): Promise<void> {
    await Promise.all([knowledgeStatus.load(), knowledgeDocs.list.refresh({ quiet: true }), knowledgeHeld.list.refresh({ quiet: true })]);
  }
</script>

<div class="relative flex size-full min-h-0 flex-col overflow-clip">
  <KnowledgeHeader slot={headerActions} {available} />

  {#if unavailable}
    <KnowledgeUnavailable availability={unavailable} onretry={() => void list.refresh()} onrefresh={refresh} />
  {:else}
    <KnowledgeOffNotice />

    <KnowledgeOfflineBanner />

    <KnowledgeEmbedNotice />

    {#if viewState.knowledgeTab === 'review'}
      <KnowledgeReview />
    {:else}
      <KnowledgeDocs />
    {/if}
  {/if}

  {#if available}
    <DropOverlay />
  {/if}
</div>

<ImportDialog />
