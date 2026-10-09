<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';
  import Orbit from '@lucide/svelte/icons/orbit';
  import { onMount } from 'svelte';

  import { scrollMemory } from '$lib/a11y/scroll-memory';
  import Button from '$lib/components/Button.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import Skeleton from '$lib/components/Skeleton.svelte';
  import ViewHeader from '$lib/components/ViewHeader.svelte';
  import RefreshButton from '$lib/components/RefreshButton.svelte';
  import { dashboard } from '$lib/stores/dashboard.svelte';
  import { ecosystem } from '$lib/stores/ecosystem.svelte';

  import DashboardSection from './dashboard/DashboardSection.svelte';
  import EcosystemCard from './EcosystemCard.svelte';
  import EcosystemHero from './EcosystemHero.svelte';
  import { installedSummary } from './ecosystem-items';

  const items = $derived(ecosystem.status?.items ?? null);
  const failed = $derived(ecosystem.state === 'error' && items === null);
  const refreshing = $derived(ecosystem.state === 'loading' || dashboard.usage.status === 'loading');

  const context = $derived.by(() => {
    if (!items) {
      return failed ? 'Could not look for companions' : 'Looking for companions…';
    }

    return dashboard.availability?.type === 'available' ? `${installedSummary(items)} · dashboard connected` : installedSummary(items);
  });

  function refresh(): void {
    void ecosystem.load(true);
    void dashboard.openPanel(true);
  }

  onMount(() => {
    void ecosystem.load();
  });
</script>

<div class="@container flex size-full min-h-0 flex-col">
  <ViewHeader title="Ecosystem" icon={Orbit} {context}>
    {#snippet actions()}
      <RefreshButton label="Refresh ecosystem" busy={refreshing} onclick={refresh} />
    {/snippet}
  </ViewHeader>

  <div use:scrollMemory={'ecosystem'} class="min-h-0 flex-1 overflow-y-auto">
    <div class="page-wide flex flex-col gap-6 py-6">
      <EcosystemHero {items} {failed} />

      <div class="grid gap-6 @6xl:grid-cols-[minmax(0,1fr)_23rem] @6xl:items-start">
        <div class="min-w-0"><DashboardSection /></div>

        <section aria-labelledby="companions-title" class="flex min-w-0 flex-col gap-4">
          <h2 id="companions-title" class="text-xl text-fg">Companions</h2>

          {#if items}
            <div class="grid gap-4 @3xl:grid-cols-3 @6xl:grid-cols-1">
              {#each items as item, index (item.id)}
                <EcosystemCard {item} {index} />
              {/each}
            </div>
          {:else if failed}
            <div class="rounded-lg border border-line bg-surface-1 shadow-raised">
              <EmptyState icon={CircleAlert} title="Could not look for companions" body={ecosystem.error ?? undefined}>
                {#snippet action()}
                  <Button variant="primary" onclick={() => void ecosystem.load(true)}>Try again</Button>
                {/snippet}
              </EmptyState>
            </div>
          {:else}
            <div class="grid gap-4 @3xl:grid-cols-3 @6xl:grid-cols-1" role="status" aria-label="Looking for companions">
              {#each [0, 1, 2] as placeholder (placeholder)}
                <Skeleton height="14rem" class="rounded-lg" />
              {/each}
            </div>
          {/if}
        </section>
      </div>
    </div>
  </div>
</div>
