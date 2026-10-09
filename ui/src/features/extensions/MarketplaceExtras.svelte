<script lang="ts">
  import CircleAlert from '@lucide/svelte/icons/circle-alert';

  import Badge from '$lib/components/Badge.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import VirtualList from '$lib/components/VirtualList.svelte';
  import type { MarketplaceEntry } from '$lib/ipc/bindings/MarketplaceEntry';
  import type { MarketplaceInfo } from '$lib/ipc/bindings/MarketplaceInfo';

  interface Props {
    marketplace: MarketplaceInfo;
  }

  let { marketplace }: Props = $props();

  /** Above this many entries the catalog scrolls in a virtualized list. */
  const VIRTUALIZE_ABOVE = 100;
  const ROW_HEIGHT = 64;

  let query = $state('');

  const entries = $derived(
    marketplace.entries.filter((entry) => `${entry.name} ${entry.description ?? ''}`.toLowerCase().includes(query.trim().toLowerCase())),
  );
</script>

{#if marketplace.error}
  <div class="flex items-start gap-3 rounded-md border border-line bg-danger-wash p-4 text-sm text-fg" role="alert">
    <CircleAlert class="mt-0.5 shrink-0 text-danger" size={16} aria-hidden="true" />
    <div class="flex flex-col gap-1">
      <p class="font-medium">The catalog could not be read</p>
      <p class="text-fg-2">{marketplace.error}</p>
    </div>
  </div>
{:else}
  <section class="flex flex-col gap-3" aria-label="Catalog">
    <div class="flex items-center justify-between gap-3">
      <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">Catalog · {marketplace.entries.length}</h3>
      {#if marketplace.entries.length > 0}
        <SearchField bind:value={query} label="Filter catalog" placeholder="Filter catalog" class="w-60" />
      {/if}
    </div>

    {#if marketplace.entries.length === 0}
      <p class="text-sm text-fg-3">This marketplace lists no plugins.</p>
    {:else if entries.length === 0}
      <p class="text-sm text-fg-3" role="status">No catalog entries match.</p>
    {:else if entries.length > VIRTUALIZE_ABOVE}
      <VirtualList items={entries} estimate={ROW_HEIGHT} label="Catalog entries" key={(entry) => entry.name} class="h-96 rounded-lg border border-line bg-surface-1">
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
    {#if entry.installed}
      <Badge tone="success">Installed</Badge>
    {/if}
  </div>
{/snippet}
