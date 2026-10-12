<script lang="ts">
  import ConfigTree from '$lib/components/ConfigTree.svelte';
  import ConfigValueView from '$lib/components/ConfigValueView.svelte';
  import { t } from '$lib/i18n/i18n.svelte';
  import type { McpServerInfo } from '$lib/ipc/bindings/McpServerInfo';

  interface Props {
    server: McpServerInfo;
  }

  let { server }: Props = $props();
</script>

{#if server.args.length > 0}
  <section class="flex flex-col gap-2" aria-label={t('extensions.mcp.arguments')}>
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('extensions.mcp.arguments')}</h3>
    <ul class="flex flex-wrap gap-1.5 font-mono text-sm">
      {#each server.args as arg, index (index)}
        <li class="rounded-sm bg-surface-2 px-2 py-0.5"><ConfigValueView value={arg} expanded /></li>
      {/each}
    </ul>
  </section>
{/if}

{#if server.url}
  <section class="flex flex-col gap-2" aria-label={t('extensions.mcp.endpoint')}>
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('extensions.mcp.endpoint')}</h3>
    <div class="font-mono text-sm break-all"><ConfigValueView value={server.url} expanded /></div>
  </section>
{/if}

{#if server.details.length > 0}
  <section class="flex flex-col gap-2" aria-label={t('extensions.mcp.settings')}>
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">{t('extensions.mcp.settings')}</h3>
    <div class="rounded-lg border border-line bg-surface-1 p-2">
      <ConfigTree fields={server.details} label={t('extensions.mcp.settingsLabel', { name: server.name })} defaultDepth={2} />
    </div>
  </section>
{/if}
