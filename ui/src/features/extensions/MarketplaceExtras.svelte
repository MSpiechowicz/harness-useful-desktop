<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import FilterMenu from '$lib/components/FilterMenu.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import type { MarketplaceEntry } from '$lib/ipc/bindings/MarketplaceEntry';
  import type { MarketplaceInfo } from '$lib/ipc/bindings/MarketplaceInfo';
  import { harness } from '$lib/stores/harness.svelte';
  import { viewState } from '$lib/stores/view-state.svelte';

  import ActionBar from './actions/ActionBar.svelte';
  import EntryInstall from './actions/EntryInstall.svelte';
  import { marketplaceActions } from './actions/item-actions';

  interface Props {
    marketplace: MarketplaceInfo;
    /** Installed plugins of this marketplace that have a newer version, by name: the catalog copy may be older than that. */
    updates: Record<string, string | null>;
  }

  let { marketplace, updates }: Props = $props();

  // The state of a catalog entry is what the row is for, so its badge is a size up from the ones that only label.
  const STATE = 'px-2.5 py-1 text-sm';

  /** Above this many entries the catalog scrolls in a virtualized list. */
  const VIRTUALIZE_ABOVE = 100;
  const ROW_HEIGHT = 64;

  type Show = 'all' | 'available' | 'installed';

  let show = $state<Show>('all');

  const id = $derived(harness.active?.info.id);

  // Filtering by install state only means something while the catalog has both kinds.
  const filterable = $derived(marketplace.entries.some((entry) => entry.installed) && marketplace.entries.some((entry) => !entry.installed));
  const filter = $derived<Show>(filterable ? show : 'all');
  const choices = $derived<{ value: Show; label: string }[]>([
    { value: 'all', label: t('extensions.catalog.all') },
    { value: 'available', label: t('extensions.catalog.available') },
    { value: 'installed', label: t('extensions.catalog.installed') },
  ]);

  const matching = $derived.by(() => {
    const needle = viewState.extensionsQuery.trim().toLowerCase();
    const found = marketplace.entries.filter((entry) => `${entry.name} ${entry.description ?? ''}`.toLowerCase().includes(needle));

    // The one search field of the view looks inside the catalog; when it matched the marketplace itself, the whole catalog shows.
    return needle === '' || found.length === 0 ? marketplace.entries : found;
  });

  const entries = $derived(matching.filter((entry) => filter === 'all' || entry.installed === (filter === 'installed')));
</script>

{#if id}
  <ActionBar actions={marketplaceActions(id, marketplace, Object.keys(updates).length > 0)} />
{/if}

{#if marketplace.error}
  <div class="flex items-start gap-3 rounded-md border border-line bg-danger-wash p-4 text-sm text-fg" role="alert">
    <CircleAlert class="mt-0.5 shrink-0 text-danger" size={16} aria-hidden="true" />
    <div class="flex flex-col gap-1">
      <p class="font-medium">{t('extensions.catalog.unreadable')}</p>
      <p class="text-fg-2">{marketplace.error}</p>
    </div>
  </div>
{:else}
  <section class="flex flex-col gap-3" aria-label={t('extensions.catalog.label')}>
    <div class="flex items-center justify-between gap-3">
      <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('extensions.catalog.label')}</h3>

      {#if filterable}
        <FilterMenu label={t('extensions.catalog.show')} options={choices} value={filter} onchange={(next) => (show = next)} />
      {/if}
    </div>

    {#if marketplace.entries.length === 0}
      <p class="text-sm text-fg-3">{t('extensions.catalog.none')}</p>
    {:else if entries.length === 0}
      <p class="text-sm text-fg-3" role="status">{t('extensions.catalog.noMatch')}</p>
    {:else if entries.length > VIRTUALIZE_ABOVE}
      <VirtualList items={entries} estimate={ROW_HEIGHT} label={t('extensions.catalog.entries')} key={(entry) => entry.name} class="h-96 rounded-lg border border-line bg-surface-1">
        {#snippet row(entry)}
          {@render entryRow(entry)}
        {/snippet}
      </VirtualList>
    {:else}
      <ul class="flex flex-col divide-y divide-line rounded-lg border border-line bg-surface-1">
        {#each entries as entry (entry.name)}
          <li>{@render entryRow(entry)}</li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

{#snippet entryRow(entry: MarketplaceEntry)}
  <div class="flex h-16 items-center gap-3 px-4">
    <div class="flex min-w-0 flex-1 flex-col">
      <span class="truncate font-mono text-sm text-fg">{entry.name}</span>
      {#if entry.description}
        <span class="truncate text-xs text-fg-2">{entry.description}</span>
      {/if}
    </div>
    {#if entry.version}
      <span class="tabular text-xs text-fg-3">v{entry.version}</span>
    {/if}
    {#if entry.name in updates}
      <Badge tone="info" class={STATE}>{updates[entry.name] ? t('extensions.badge.updateTo', { version: updates[entry.name] }) : t('extensions.badge.updateAvailable')}</Badge>
    {:else if entry.installed}
      <Badge tone="success" class={STATE}>{t('extensions.catalog.installed')}</Badge>
    {:else if !marketplace.error}
      <EntryInstall marketplace={marketplace.name} entry={entry.name} />
    {/if}
  </div>
{/snippet}
