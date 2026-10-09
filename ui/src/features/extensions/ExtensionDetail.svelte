<script lang="ts">
  import { detailBasics } from './extension-detail';
  import type { ExtItem } from './extension-items';
  import DetailShell from './DetailShell.svelte';
  import MarketplaceExtras from './MarketplaceExtras.svelte';
  import McpExtras from './McpExtras.svelte';
  import PackageExtras from './PackageExtras.svelte';
  import PluginExtras from './PluginExtras.svelte';

  interface Props {
    entry: ExtItem;
  }

  let { entry }: Props = $props();

  const basics = $derived(detailBasics(entry));
</script>

<DetailShell title={entry.title} badges={entry.badges} description={basics.description} facts={basics.facts}>
  {#if entry.tab === 'plugins'}
    <PluginExtras plugin={entry.data} />
  {:else if entry.tab === 'marketplaces'}
    <MarketplaceExtras marketplace={entry.data} />
  {:else if entry.tab === 'mcp'}
    <McpExtras server={entry.data} />
  {:else if entry.tab === 'packages'}
    <PackageExtras pkg={entry.data} />
  {/if}
</DetailShell>
