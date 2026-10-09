<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import ConfigValueView from '$lib/components/ConfigValueView.svelte';
  import type { PackageInfo } from '$lib/ipc/bindings/PackageInfo';

  interface Props {
    pkg: PackageInfo;
  }

  let { pkg }: Props = $props();
</script>

<section class="flex flex-col gap-2" aria-label="Source">
  <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">Source</h3>
  <div class="font-mono text-sm break-all"><ConfigValueView value={pkg.source} expanded /></div>
</section>

{#if pkg.resources.length > 0}
  <section class="flex flex-col gap-2" aria-label="Provides">
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">Provides</h3>
    <div class="flex flex-wrap gap-1.5">
      {#each pkg.resources as resource (resource)}
        <Badge tone="accent">{resource}</Badge>
      {/each}
    </div>
  </section>
{/if}
