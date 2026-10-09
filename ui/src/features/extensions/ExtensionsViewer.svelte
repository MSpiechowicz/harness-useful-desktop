<script lang="ts">
  import Blocks from '@lucide/svelte/icons/blocks';
  import RotateCw from '@lucide/svelte/icons/rotate-cw';

  import EmptyState from '$lib/components/EmptyState.svelte';
  import HarnessBadge from '$lib/components/HarnessBadge.svelte';
  import IconButton from '$lib/components/IconButton.svelte';
  import NoticeBanner from '$lib/components/NoticeBanner.svelte';
  import Tabs from '$lib/components/Tabs.svelte';
  import { harness } from '$lib/stores/harness.svelte';
  import { project } from '$lib/stores/project.svelte';
  import { extensionsView } from '$lib/stores/views.svelte';

  import HarnessGate from '../harness/HarnessGate.svelte';
  import { tabCount, visibleTabs, type ExtTab } from './extension-items';
  import ExtensionsPanel from './ExtensionsPanel.svelte';

  let chosen = $state<ExtTab>('plugins');

  const tabs = $derived(harness.active ? visibleTabs(harness.active.capabilities) : []);
</script>

<HarnessGate loader={extensionsView} noun="extensions">
  {#snippet children(view)}
    {@const options = tabs.map((tab) => ({ value: tab.id, label: tab.label, count: tabCount(tab.id, view) }))}
    {@const current = options.some((option) => option.value === chosen) ? chosen : options[0]?.value}

    <header class="flex items-center gap-3 px-6 pt-5 pb-3">
      {#if harness.active}
        <HarnessBadge info={harness.active.info} />
      {/if}
      <div class="flex min-w-0 flex-1 flex-col">
        <h2 class="text-xl text-fg">Extensions</h2>
        <p class="truncate text-sm text-fg-2">
          {harness.active?.info.name} · {project.active ? project.active.name : 'no project open'}
        </p>
      </div>
      <IconButton icon={RotateCw} label="Reload extensions" side="bottom" onclick={() => extensionsView.refresh()} />
    </header>

    {#if view.notices.length > 0}
      <div class="px-6 pb-3"><NoticeBanner notices={view.notices} /></div>
    {/if}

    {#if current}
      <Tabs
        items={options}
        bind:value={() => current, (next) => (chosen = next)}
        label="Extension types"
        class="min-h-0 flex-1 gap-0"
        stripClass="mx-6 self-start"
        panelClass="min-h-0 flex-1 border-t border-line"
      >
        {#snippet panel(tab)}
          <ExtensionsPanel {tab} {view} />
        {/snippet}
      </Tabs>
    {:else}
      <div class="grid flex-1 place-items-center">
        <EmptyState icon={Blocks} title="No extension types" body="This harness does not expose plugins, skills or MCP servers." />
      </div>
    {/if}
  {/snippet}
</HarnessGate>
