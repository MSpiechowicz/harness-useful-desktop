<script lang="ts">
  import { detailBasics, overviewStats } from './extension-detail';
  import { TABS, type ExtItem } from './extension-items';
  import DetailShell from './DetailShell.svelte';
  import MarketplaceExtras from './MarketplaceExtras.svelte';
  import McpExtras from './McpExtras.svelte';
  import PackageExtras from './PackageExtras.svelte';
  import PluginExtras from './PluginExtras.svelte';
  import SkillExtras from './SkillExtras.svelte';

  interface Props {
    entry: ExtItem;
  }

  let { entry }: Props = $props();

  const basics = $derived(detailBasics(entry));
  const kind = $derived(TABS.find((def) => def.id === entry.tab)?.singular ?? '');
  // What the description card already says is not said again under the title.
  const subtitle = $derived(entry.subtitle && entry.subtitle !== basics.description ? `${kind} · ${entry.subtitle}` : kind);
</script>

<DetailShell title={entry.title} {subtitle} description={basics.description} facts={basics.facts} stats={overviewStats(entry)}>
  {#if entry.tab === 'plugins'}
    <PluginExtras plugin={entry.data} />
  {:else if entry.tab === 'marketplaces'}
    <MarketplaceExtras marketplace={entry.data} updates={entry.updates} />
  {:else if entry.tab === 'skills'}
    <SkillExtras skill={entry.data} />
  {:else if entry.tab === 'mcp'}
    <McpExtras server={entry.data} />
  {:else if entry.tab === 'packages'}
    <PackageExtras pkg={entry.data} />
  {/if}
</DetailShell>
