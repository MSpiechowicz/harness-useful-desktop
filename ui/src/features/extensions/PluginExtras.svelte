<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { PluginInfo } from '$lib/ipc/bindings/PluginInfo';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';

  import ActionBar from './actions/ActionBar.svelte';
  import { pluginActions } from './actions/item-actions';

  interface Props {
    plugin: PluginInfo;
  }

  let { plugin }: Props = $props();

  // A harness whose CLI the app does not drive has nothing to run, so there is no action bar.
  const id = $derived(harness.active?.capabilities.cliActions ? harness.active.info.id : undefined);
</script>

{#if id}
  <ActionBar actions={pluginActions(id, plugin, project.active?.path ?? null)} />
{/if}

{#if plugin.features.length > 0}
  <section class="flex flex-col gap-2" aria-label={t('extensions.plugin.features')}>
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('extensions.plugin.features')}</h3>
    <ul class="flex flex-col divide-y divide-line rounded-lg border border-line bg-surface-1">
      {#each plugin.features as feature (feature.name)}
        <li class="flex items-center justify-between gap-3 px-4 py-2.5 text-sm">
          <span class="font-mono text-fg">{feature.name}</span>
          <Badge tone={feature.enabled ? 'success' : 'neutral'}>{feature.enabled ? t('extensions.state.enabled') : t('extensions.state.disabled')}</Badge>
        </li>
      {/each}
    </ul>
  </section>
{/if}
