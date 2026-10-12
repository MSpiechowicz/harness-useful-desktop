<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import { onMount } from 'svelte';

  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import { dashboard } from '$lib/stores/dashboard.svelte';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';

  import DashboardInstallCard from './DashboardInstallCard.svelte';
  import DashboardPanel from './DashboardPanel.svelte';

  let checking = $state(false);

  const availability = $derived(dashboard.availability);
  const item = $derived(ecosystem.status?.items.find((candidate) => candidate.id === 'dashboard'));

  async function recheck(): Promise<void> {
    checking = true;

    try {
      await Promise.all([ecosystem.load(true), dashboard.openPanel(true)]);
    } finally {
      checking = false;
    }
  }

  onMount(() => {
    void dashboard.openPanel();
  });
</script>

{#if availability === null}
  <div class="flex flex-col gap-4" role="status" aria-label={t('ecosystem.section.checking')}>
    <Skeleton width="12rem" height="1.75rem" />
    <div class="grid gap-3 @xl:grid-cols-2">
      <Skeleton height="7.5rem" class="rounded-lg" />
      <Skeleton height="7.5rem" class="rounded-lg" />
    </div>
    <Skeleton height="14rem" class="rounded-lg" />
  </div>
{:else if availability.type === 'notInstalled'}
  <DashboardInstallCard {item} {checking} onrecheck={() => void recheck()} />
{:else if availability.type === 'failed'}
  <div class="rounded-lg border border-line bg-surface-1 shadow-raised">
    <EmptyState icon={CircleAlert} title={t('ecosystem.section.failed')} body={availability.message}>
      {#snippet action()}
        <Button variant="primary" loading={checking} onclick={() => void recheck()}>{t('ecosystem.retry')}</Button>
      {/snippet}
    </EmptyState>
  </div>
{:else}
  <DashboardPanel />
{/if}
