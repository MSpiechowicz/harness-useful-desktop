<script lang="ts">
  import SegmentedControl from '$lib/components/SegmentedControl.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { knowledgeChanges } from '$lib/stores/knowledgeChanges.svelte';
  import { knowledgeHeld } from '$lib/stores/knowledgeHeld.svelte';
  import { knowledgeDocs } from '$lib/stores/knowledgeDocs.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  interface Props {
    /** The list was chosen: the open row of the other one is let go of. */
    onchange?: () => void;
  }

  let { onchange }: Props = $props();

  // A count is only said once it is known. The database says how many of each there are (live), the pages do until it has.
  const documents = $derived(knowledgeChanges.pulse?.documents);
  const proposals = $derived(knowledgeChanges.pulse?.held ?? knowledgeHeld.list.data?.total);
  const options = $derived([
    { value: 'docs' as const, label: t('knowledge.tab.documents'), count: documents ?? (viewState.knowledgeQuery === '' && viewState.knowledgeScope === 'all' && viewState.knowledgeFormat === null && viewState.knowledgePlace === null ? knowledgeDocs.list.data?.total : undefined) },
    { value: 'review' as const, label: t('knowledge.tab.review'), count: proposals || undefined },
  ]);
</script>

<SegmentedControl {options} bind:value={viewState.knowledgeTab} label={t('knowledge.tab.label')} {onchange} />
