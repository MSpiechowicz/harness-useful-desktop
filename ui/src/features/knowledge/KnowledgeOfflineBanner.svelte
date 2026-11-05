<script lang="ts">
  import OfflineBanner from '$lib/components/OfflineBanner.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';

  // The live counts say it first and say when it is over. Until they came, the list on screen does.
  const offline = $derived(knowledgeChanges.pulse?.offline ?? knowledgeDocs.list.data?.offline ?? false);
</script>

<OfflineBanner {offline} message={t('knowledge.offline.title')} detail={t('knowledge.offline.paused')} reason={knowledgeChanges.pulse?.offlineReason} />
