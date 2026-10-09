<script lang="ts">
  import PackageOpen from '@lucide/svelte/icons/package-open';

  import EmptyState from '$lib/components/EmptyState.svelte';
  import ListDetail from '$lib/components/ListDetail.svelte';
  import SearchField from '$lib/components/SearchField.svelte';
  import type { ExtensionsView } from '$lib/ipc/bindings/ExtensionsView';
  import { viewState } from '$lib/stores/view-state.svelte';

  import ExtensionDetail from './ExtensionDetail.svelte';
  import ExtensionList from './ExtensionList.svelte';
  import { buildItems, filterItems, TABS, type ExtTab } from './extension-items';

  interface Props {
    tab: ExtTab;
    view: ExtensionsView;
  }

  let { tab, view }: Props = $props();

  const def = $derived(TABS.find((candidate) => candidate.id === tab) ?? TABS[0]);

  const all = $derived(buildItems(tab, view));
  const items = $derived(filterItems(all, viewState.extensionsQuery));
  const selected = $derived(items.find((entry) => entry.id === viewState.extensionsItem) ?? items[0]);
</script>

{#if all.length === 0}
  <div class="grid h-full place-items-center">
    <EmptyState icon={PackageOpen} title="No {def.noun} found" body="Nothing is configured for this harness in the current scope." />
  </div>
{:else}
  <ListDetail label="Resize list" scrollKey="extensions:detail">
    {#snippet list()}
      <div class="p-3">
        <SearchField bind:value={viewState.extensionsQuery} label="Search {def.noun}" placeholder="Search {def.noun}" />
      </div>

      {#if items.length === 0}
        <p class="px-4 py-6 text-center text-sm text-fg-3" role="status">No {def.noun} match “{viewState.extensionsQuery.trim()}”.</p>
      {:else}
        <ExtensionList {items} selectedId={selected?.id ?? ''} label={def.label} onselect={(id) => (viewState.extensionsItem = id)} />
      {/if}
    {/snippet}

    {#snippet detail()}
      {#if selected}
        {#key selected.id}
          <ExtensionDetail entry={selected} />
        {/key}
      {/if}
    {/snippet}
  </ListDetail>
{/if}
