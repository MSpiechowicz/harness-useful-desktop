<script lang="ts">
  import { t } from '$lib/i18n/i18n.svelte';
  import { updates } from '$lib/stores/updates.svelte';

  import { progressLabel } from './copy';
  import UpdateBar from './UpdateBar.svelte';

  /** The label and size above the update's progress bar. */
  const state = $derived(updates.state);
  const title = $derived(state.kind === 'installing' ? t('updates.progress.installing') : t('updates.progress.downloading'));
  const detail = $derived(state.kind === 'installing' ? '' : progressLabel(state));
</script>

<div class="flex flex-col gap-2">
  <div class="flex items-baseline justify-between gap-3 text-sm">
    <span class="text-fg-2">{title}</span>
    <span class="tabular text-fg-3">{detail}</span>
  </div>
  <UpdateBar />
</div>
