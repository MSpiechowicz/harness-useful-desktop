<script lang="ts">
  import { fade } from 'svelte/transition';

  import { rise, scaled } from '$lib/design/motion';
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
  import ReviewChangesView from './ReviewChangesView.svelte';
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

<!-- The review and the lists share one grid cell and crossfade in place, like the views of the shell: the outgoing one stays
     while its top bar header fades out, so in a column the incoming one would start under it and then jump up. -->
<div class="grid size-full min-h-0 *:col-start-1 *:row-start-1">
  {#key viewState.knowledgeReviewId !== null}
  <div class="relative flex size-full min-h-0 flex-col overflow-clip" in:rise={{ y: 10, delay: 80, duration: 320 }} out:fade={{ duration: scaled(120) }}>
  {#if viewState.knowledgeReviewId !== null}
    <!-- The full-width review of the pending changes of one document replaces the lists until it is left. -->
    <ReviewChangesView id={viewState.knowledgeReviewId} onclose={() => (viewState.knowledgeReviewId = null)} />
  {:else}
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
  {/if}
  </div>
  {/key}
</div>

<ImportDialog />
