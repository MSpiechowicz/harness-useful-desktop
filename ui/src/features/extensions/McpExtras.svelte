<script lang="ts">
  import ConfigTree from '$lib/components/ConfigTree.svelte';
  import ConfigValueView from '$lib/components/ConfigValueView.svelte';
  import type { McpServerInfo } from '$lib/ipc/bindings/McpServerInfo';

  interface Props {
    server: McpServerInfo;
  }

  let { server }: Props = $props();
</script>

{#if server.args.length > 0}
  <section class="flex flex-col gap-2" aria-label="Arguments">
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">Arguments</h3>
    <ul class="flex flex-wrap gap-1.5 font-mono text-sm">
      {#each server.args as arg, index (index)}
        <li class="rounded-sm bg-surface-2 px-2 py-0.5"><ConfigValueView value={arg} expanded /></li>
      {/each}
    </ul>
  </section>
{/if}

{#if server.url}
  <section class="flex flex-col gap-2" aria-label="Endpoint">
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">Endpoint</h3>
    <div class="font-mono text-sm break-all"><ConfigValueView value={server.url} expanded /></div>
  </section>
{/if}

{#if server.details.length > 0}
  <section class="flex flex-col gap-2" aria-label="Settings">
    <h3 class="text-sm font-medium tracking-wider text-fg-3 uppercase">Settings</h3>
    <div class="rounded-lg border border-line bg-surface-1 p-2">
      <ConfigTree fields={server.details} label="{server.name} settings" defaultDepth={2} />
    </div>
  </section>
{/if}
