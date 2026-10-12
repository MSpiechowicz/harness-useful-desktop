<script lang="ts">
  import Badge from '$lib/components/Badge.svelte';
  import ConfigValueView from '$lib/components/ConfigValueView.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { PackageInfo } from '$lib/ipc/bindings/PackageInfo';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';

  import ActionBar from './actions/ActionBar.svelte';
  import { packageActions } from './actions/item-actions';

  interface Props {
    pkg: PackageInfo;
  }

  let { pkg }: Props = $props();

  const actions = $derived(harness.active ? packageActions(harness.active.info.id, pkg, project.active?.path ?? null) : []);
</script>

{#if actions.length > 0}
  <ActionBar {actions} />
{/if}

<section class="flex flex-col gap-2" aria-label={t('extensions.package.source')}>
  <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('extensions.package.source')}</h3>
  <div class="font-mono text-sm break-all"><ConfigValueView value={pkg.source} expanded /></div>
</section>

{#if pkg.resources.length > 0}
  <section class="flex flex-col gap-2" aria-label={t('extensions.package.provides')}>
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('extensions.package.provides')}</h3>
    <div class="flex flex-wrap gap-1.5">
      {#each pkg.resources as resource (resource)}
        <Badge tone="accent">{resource}</Badge>
      {/each}
    </div>
  </section>
{/if}
