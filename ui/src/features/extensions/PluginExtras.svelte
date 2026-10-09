<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import type { PluginInfo } from '$lib/ipc/bindings/PluginInfo';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';

  import ActionBar from './actions/ActionBar.svelte';
  import { pluginActions } from './actions/item-actions';

  interface Props {
    plugin: PluginInfo;
  }

  let { plugin }: Props = $props();

  const id = $derived(harness.active?.info.id);
</script>

{#if id}
  <ActionBar actions={pluginActions(id, plugin, project.active?.path ?? null)} />
{/if}

{#if plugin.features.length > 0}
  <section class="flex flex-col gap-2" aria-label="Features">
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">Features</h3>
    <ul class="flex flex-col divide-y divide-line rounded-lg border border-line bg-surface-1">
      {#each plugin.features as feature (feature.name)}
        <li class="flex items-center justify-between gap-3 px-4 py-2.5 text-sm">
          <span class="font-mono text-fg">{feature.name}</span>
          <Badge tone={feature.enabled ? 'success' : 'neutral'}>{feature.enabled ? 'Enabled' : 'Disabled'}</Badge>
        </li>
      {/each}
    </ul>
  </section>
{/if}
